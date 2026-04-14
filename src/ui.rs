use std::{
    env,
    sync::{
        mpsc::{Receiver, Sender},
    },
    time::Duration,
};

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Alignment, Constraint, Layout},
    style::Stylize,
    text::{Line, Text},
    widgets::{Block, List, ListItem, ListState, Paragraph},
};
use thiserror::Error;

use crate::client::{ClientState};

#[derive(Error, Debug)]
pub enum UIError {
    #[error("Error while drawing in the terminal: {0}")]
    TerminalError(String),
    #[error("Error while handling an event: {0}")]
    EventError(String),
}

#[derive(Debug, Clone)]
pub enum AppCommand {
    Start,
    Stop,
    Exit,
}

pub struct AppUI {
    input: String,
    pub exit: bool,
    cached_state: ClientState,
    logs: Vec<String>,
    list_state: ListState,
    log_rx: Receiver<String>,
    cmd_tx: Sender<AppCommand>,
    state_rx: Receiver<ClientState>
}

impl AppUI {
    pub fn new(
        log_rx: Receiver<String>,
        cmd_tx: Sender<AppCommand>,
        state_rx: Receiver<ClientState>
    ) -> Self {
        AppUI {
            input: String::new(),
            exit: false,
            cached_state: ClientState::Idle,
            logs: Vec::new(),
            list_state: ListState::default(),
            log_rx,
            cmd_tx,
            state_rx,
        }
    }

    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> Result<(), UIError> {
        let secs_to_shutdown = 3;
        let mut countdown = secs_to_shutdown;
        let mut countdown_started = false;

        while !self.exit {
            if let Ok(state) = self.state_rx.try_recv() {
                self.cached_state = state;
            }

            terminal
                .draw(|frame| {
                    if let Err(e) = self.render_ui(frame) {
                        self.push_log(format!("UI Error: {}", e));
                    }
                })
                .map_err(|e| UIError::TerminalError(e.to_string()))?;

            if self.cached_state == ClientState::Exit {
                let should_exit = self.should_exit(&mut countdown, countdown_started);

                if countdown_started {
                    std::thread::sleep(Duration::from_secs(1));
                }

                self.exit = should_exit;
                countdown = countdown.saturating_sub(1);

                countdown_started = true;
                continue;
            }

            self.handle_input()?;
            if let Ok(log) = self.log_rx.try_recv() {
                self.push_log(log);
            }
        }
        Ok(())
    }

    fn render_ui(&mut self, frame: &mut Frame) -> Result<(), UIError> {
        let layout = Layout::vertical([
            Constraint::Length(4),
            Constraint::Fill(1),
            Constraint::Length(4),
        ])
        .split(frame.area());

        // Title
        let version = env!("CARGO_PKG_VERSION");
        let title = Paragraph::new(Text::from(vec![
            Line::from("Atlas Observer".bold()),
            Line::from(version.dim()),
        ]))
        .alignment(Alignment::Center)
        .block(Block::bordered());
        frame.render_widget(title, layout[0]);

        // Logs
        let log_items: Vec<ListItem> = self
            .logs
            .iter()
            .map(|l| ListItem::from(format!("[LOG] {}", l.trim())))
            .collect();
        let logs = List::new(log_items).block(Block::bordered());
        frame.render_stateful_widget(logs, layout[1], &mut self.list_state);

        // Input
        let client_state = &self.cached_state;

        let commands = match *client_state {
            ClientState::Idle => "Commands: start | exit",
            ClientState::WaitingForOpponent => "Commands: stop | exit",
            ClientState::PlayingRanked(_) => "Commands: exit (please, close MBAA first)",
            _ => "Commands: stop | exit",
        };
        let cmd_input = Paragraph::new(Text::from(vec![
            Line::from(commands),
            Line::from(format!("> {}", self.input)),
        ]))
        .block(Block::bordered());
        frame.render_widget(cmd_input, layout[2]);

        let cursor_x = layout[2].x + 3 + self.input.len() as u16;
        let cursor_y = layout[2].y + 2;
        frame.set_cursor_position((cursor_x, cursor_y));

        Ok(())
    }

    pub fn handle_input(&mut self) -> Result<(), UIError> {
        if event::poll(Duration::from_millis(16)).map_err(|e| UIError::EventError(e.to_string()))? {
            if let Event::Key(key) =
                event::read().map_err(|e| UIError::EventError(e.to_string()))?
            {
                if key.kind != KeyEventKind::Press {
                    return Ok(());
                }
                match key.code {
                    KeyCode::Char('x') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.send_app_cmd(AppCommand::Exit);
                    }
                    KeyCode::Char('v') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        let content = cli_clipboard::get_contents()
                            .map_err(|_| UIError::EventError("paste".to_string()))?;
                        self.input.push_str(content.as_str());
                    }
                    KeyCode::Enter => {
                        let cmd = self.input.trim().to_string();
                        if !cmd.is_empty() {
                            self.handle_cmd(cmd)?;
                        }
                        self.input.clear();
                    }
                    KeyCode::Backspace => {
                        self.input.pop();
                    }
                    KeyCode::Char(c) => {
                        self.input.push(c);
                    }
                    KeyCode::Up => {
                        self.list_state.scroll_up_by(1);
                    }
                    KeyCode::Down => {
                        self.list_state.scroll_down_by(1);
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }

    fn handle_cmd(&mut self, cmd: String) -> Result<(), UIError> {
        let is_idle = self.cached_state == ClientState::Idle;
        let can_stop = self.cached_state == ClientState::WaitingForOpponent;
        let is_playing = matches!(self.cached_state, ClientState::PlayingRanked(_));

        match cmd.as_str() {
            "start" if is_idle => self.send_app_cmd(AppCommand::Start),
            "stop" if can_stop => self.send_app_cmd(AppCommand::Stop),
            "exit" if !is_playing => self.send_app_cmd(AppCommand::Exit),
            cmd => self.push_log(format!("Invalid command '{}'", cmd)),
        }

        Ok(())
    }

    fn send_app_cmd(&mut self, app_cmd: AppCommand) {
        if let Err(e) = self.cmd_tx.send(app_cmd) {
            self.push_log(format!("Command not received: {}", e));
        }
    }

    pub fn push_log(&mut self, log: String) {
        self.logs.push(log);
    }

    fn should_exit(&mut self, countdown: &mut u8, countdown_started: bool) -> bool {
        if !countdown_started {
            self.push_log(String::new());
        }
        let last_index = self.logs.len() - 1;
        self.logs[last_index] = format!("Atlas will be closing in {} seconds", countdown);
        *countdown == 0
    }
}
