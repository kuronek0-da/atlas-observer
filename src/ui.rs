use std::{
    sync::{Arc, Mutex, MutexGuard, mpsc::{Receiver, Sender}},
    time::Duration,
};

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Alignment, Constraint, Layout},
    style::Stylize,
    text::{Line, Text},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};
use thiserror::Error;

use crate::client::{ClientState, http::ClientError};

#[derive(Error, Debug)]
pub enum UIError {
    #[error("Error while drawing in the terminal: {0}")]
    TerminalError(String),
    #[error("Error while handling an event: {0}")]
    EventError(String),
}

pub enum AppCommand {
    Host(ClientState),
    Join(ClientState),
    Stop(ClientState),
    Exit,
}

pub struct AppUI {
    input: String,
    pub exit: bool,
    client_state: Arc<Mutex<ClientState>>,
    logs: Vec<String>,
    list_state: ListState,
    log_rx: Receiver<String>,
    cmd_tx: Sender<AppCommand>,
}

impl AppUI {
    pub fn new(log_rx: Receiver<String>, cmd_tx: Sender<AppCommand>, client_state: Arc<Mutex<ClientState>>) -> Self {
        AppUI {
            input: String::new(),
            exit: false,
            client_state,
            logs: Vec::new(),
            list_state: {
                let mut s = ListState::default();
                s.select_first();
                s
            },
            log_rx,
            cmd_tx,
        }
    }

    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> Result<(), UIError> {
        while !self.exit {
            terminal
                .draw(|frame| self.render_ui(frame))
                .map_err(|e| UIError::TerminalError(e.to_string()))?;
            self.handle_input()?;
            if let Ok(log) = self.log_rx.try_recv() {
                self.push_log(log);
            }
        }
        Ok(())
    }

    fn render_ui(&mut self, frame: &mut Frame) {
        let layout = Layout::vertical([
            Constraint::Length(4),
            Constraint::Fill(1),
            Constraint::Length(4),
        ])
        .split(frame.area());

        // Title
        let title = Paragraph::new(Text::from(vec![
            Line::from("Atlas Observer".bold()),
            Line::from("v0.1.0-alpha".dim()),
        ]))
        .alignment(Alignment::Center)
        .block(Block::new().borders(Borders::ALL));
        frame.render_widget(title, layout[0]);

        // Logs
        let log_items: Vec<ListItem> = self
            .logs
            .iter()
            .map(|l| ListItem::from(l.as_str()))
            .collect();
        let logs = List::new(log_items).block(Block::new().borders(Borders::ALL));
        frame.render_stateful_widget(logs, layout[1], &mut self.list_state);

        // Input
        let client_state = self.client_state().unwrap();
        let commands = match *client_state {
            ClientState::Idle => "Commands: host <opt code> | join <code> | exit",
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
    }

    fn client_state(&self) -> Result<MutexGuard<'_, ClientState>, ClientError> {
        self.client_state.lock().map_err(|_| ClientError::StateError)
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
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.exit = true;
                    }
                    KeyCode::Enter => {
                        let cmd = self.input.trim().to_string();
                        if !cmd.is_empty() {
                            self.handle_cmd(cmd);
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

    fn handle_cmd(&mut self, cmd: String) {
        let is_idle = *self.client_state().unwrap() == ClientState::Idle;
        match cmd.as_str() {
            "host" if is_idle => {
                self.send_app_cmd(AppCommand::Host(ClientState::hosting()))
            }
            "stop" if !is_idle => self.send_app_cmd(AppCommand::Stop(ClientState::Idle)),
            "exit" => self.send_app_cmd(AppCommand::Exit),
            cmd if cmd.starts_with("host ") && is_idle => self.send_app_cmd(
                AppCommand::Host(ClientState::HostingRanked(cmd[5..].to_string())),
            ),
            cmd if cmd.starts_with("join ") && is_idle => self.send_app_cmd(
                AppCommand::Join(ClientState::JoinedRanked(cmd[5..].to_string())),
            ),
            cmd => self.push_log(format!("Invalid command '{}'", cmd)),
        }
    }

    fn update_state(&mut self, client_state: ClientState) {
        let mut data = self.client_state.lock().unwrap();
        *data = client_state;
    }

    fn send_app_cmd(&mut self, app_cmd: AppCommand) {
        let client_state = match &app_cmd {
            AppCommand::Host(state) => state.clone(),
            AppCommand::Join(state) => state.clone(),
            AppCommand::Stop(state) => state.clone(),
            _ => {
                self.exit = true;
                // app will exit
                ClientState::Idle
            },
        };
        self.update_state(client_state);

        match self.cmd_tx.send(app_cmd) {
            Ok(_) => {}
            Err(e) => self.push_log(format!("Command not received: {}", e)),
        }
    }

    pub fn push_log(&mut self, log: String) {
        self.logs.push(log);
        self.list_state.select_last();
    }
}
