use crate::{
    client::{ClientManager, ClientState, http::ClientError},
    game::state::GameState,
    log, memory,
    ui::AppCommand,
    validation,
};
use log::{error, info};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{Receiver, Sender, TryRecvError, channel},
    },
    thread::JoinHandle,
    time::Duration,
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum RunnerError {
    #[error(transparent)]
    StateError(#[from] ClientError),
    #[error("Failed to send/receive data, UI channel dropped")]
    UIChannelDroppedError,
    #[error("Failed to send/receive data, Memory channel dropped")]
    MemoryChannelDroppedError,
}

enum Queue {
    Matched,
    Canceled,
}

#[derive(Clone)]
struct RunnerContext {
    client: ClientManager,
    log_tx: Sender<String>,
    state_tx: Sender<ClientState>,
    is_canceled: Arc<AtomicBool>,
    are_paired: Arc<AtomicBool>,
}

impl RunnerContext {
    fn change_state(&self, state: ClientState) -> Result<(), RunnerError> {
        info!("Transitioning to state: {:?}", state);

        if let Err(e) = self.client.update_state(state.clone()) {
            error!("Failed to update state in Runner: {}", &e);
            return Err(RunnerError::from(e));
        }
        if self.state_tx.send(state).is_err() {
            let e = RunnerError::UIChannelDroppedError;
            error!("Runner Error: {}", &e);
            return Err(e);
        }
        Ok(())
    }

    fn is_canceled(&self) -> bool {
        self.is_canceled.load(Ordering::Relaxed)
    }

    fn log(&self, msg: String) {
        log(msg, &self.log_tx);
    }
}

pub fn run(
    client: ClientManager,
    log_tx: Sender<String>,
    cmd_rx: Receiver<AppCommand>,
    state_tx: Sender<ClientState>,
) {
    let ctx = RunnerContext {
        client,
        log_tx,
        state_tx,
        is_canceled: Arc::new(AtomicBool::new(false)),
        are_paired: Arc::new(AtomicBool::new(false)),
    };
    info!("Runner thread started");

    loop {
        ctx.is_canceled.store(false, Ordering::Relaxed);
        ctx.are_paired.store(false, Ordering::Relaxed);

        // ==== Waiting for 'start' command
        match wait_for_start_cmd(&ctx, &cmd_rx) {
            Some(ClientState::Exit) => break,
            Some(_) => {}
            None => continue,
        };

        if ctx.change_state(ClientState::WaitingForOpponent).is_err() {
            ctx.log("Error trying to start ranked mode, check logs for details".to_string());
            break;
        }
        ctx.log("Ranked mode enabled".to_string());

        // ==== Start searching for session ids

        let (game_state_tx, game_state_rx) = channel();
        let ids = match acquire_session_ids(&ctx, game_state_tx, &cmd_rx) {
            Ok(Some(v)) => v,
            Ok(None) => continue,
            Err(_) => {
                if ctx.change_state(ClientState::Idle).is_err() {
                    ctx.log("Failed to exit ranked mode".to_string());
                    break;
                }
                ctx.log("Ranked mode disabled".to_string());
                continue;
            }
        };

        // ==== Pairing/Queueing
        match perform_queueing(&ctx, ids, &cmd_rx) {
            Ok(Queue::Matched) => {
                validation::run(game_state_rx, &ctx.client, ctx.log_tx.clone());
            }
            Ok(Queue::Canceled) => {
                continue;
            }
            Err(e) => {
                error!("Error trying to perform queueing: {}", e);
                ctx.log("Internal error while queueing".to_string());
                break;
            }
        }
    }
    ctx.log("Atlas stopped running".to_string());
}

fn wait_for_start_cmd(ctx: &RunnerContext, cmd_rx: &Receiver<AppCommand>) -> Option<ClientState> {
    let cmd = match cmd_rx.recv() {
        Ok(cmd) => cmd,
        Err(_) => {
            error!(
                "Runner Error while waiting for start command: {}",
                RunnerError::UIChannelDroppedError
            );
            ctx.log("Error trying to receive command. Check logs for details".to_string());
            return None;
        }
    };

    let state = match cmd {
        AppCommand::Start => ClientState::WaitingForOpponent,
        AppCommand::Exit => {
            if let Err(_) = ctx.change_state(ClientState::Exit) {
                ctx.log("Internal error, could not exit Atlas".to_string());
            }
            return Some(ClientState::Exit);
        }
        _ => return None,
    };
    Some(state)
}

fn acquire_session_ids(
    ctx: &RunnerContext,
    game_state_tx: Sender<GameState>,
    cmd_rx: &Receiver<AppCommand>,
) -> Result<Option<Vec<String>>, RunnerError> {
    //let are_players_paired = Arc::clone(&ctx.are_paired);
    //let is_queue_canceled = Arc::clone(&ctx.is_canceled);

    //let state_tx_mt = ctx.state_tx.clone();
    //let log_tx_mt = ctx.log_tx.clone();
    //let (ids_tx, ids_rx) = channel();

    let ids_rx = spawn_memory_worker(ctx, game_state_tx);

    //// Attaching process
    //std::thread::spawn(move || {
    //    memory::run(
    //        game_state_tx,
    //        &log_tx_mt,
    //        ids_tx,
    //        is_queue_canceled,
    //        are_players_paired,
    //    )
    //});

    loop {
        if let Some(state) = check_cancel_cmd(&ctx, cmd_rx) {
            if ctx.change_state(state).is_err() {
                ctx.log(
                    "Internal error trying to cancel ranked mode, check logs for details"
                        .to_string(),
                );
            }
            ctx.is_canceled.store(true, Ordering::Relaxed);
            ctx.log("Ranked mode disabled".to_string());
            break Ok(None);
        }

        match ids_rx.try_recv() {
            Ok(ids) => break Ok(Some(ids)),
            Err(TryRecvError::Disconnected) => break Err(RunnerError::MemoryChannelDroppedError),
            Err(_) => {}
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}

/// Spawns memory thread and return a Receiver of Session IDs
fn spawn_memory_worker(
    ctx: &RunnerContext,
    game_state_tx: Sender<GameState>,
) -> Receiver<Vec<String>> {
    let are_players_paired = Arc::clone(&ctx.are_paired);
    let is_queue_canceled = Arc::clone(&ctx.is_canceled);

    let log_tx_mt = ctx.log_tx.clone();
    let (ids_tx, ids_rx) = channel();

    // Attaching process
    std::thread::spawn(move || {
        memory::run(
            game_state_tx,
            log_tx_mt,
            ids_tx,
            is_queue_canceled,
            are_players_paired,
        )
    });

    ids_rx
}

/// Returns an Option containing Idle or Exit if cancel was received
/// Only Stop and Exit are considered cancel commands
fn check_cancel_cmd(ctx: &RunnerContext, cmd_rx: &Receiver<AppCommand>) -> Option<ClientState> {
    match cmd_rx.try_recv() {
        Ok(AppCommand::Stop) => Some(ClientState::Idle),
        Ok(AppCommand::Exit) => Some(ClientState::Exit),
        Ok(_) => return None,
        Err(TryRecvError::Disconnected) => {
            error!(
                "Error trying to check cancel command: {}",
                RunnerError::UIChannelDroppedError
            );
            ctx.log("Error trying to receive command".to_string());
            return None;
        }
        Err(_) => return None,
    }
}

/// Sends queue request to the server and updates the client state when players match
fn perform_queueing(
    ctx: &RunnerContext,
    ids: Vec<String>,
    cmd_rx: &Receiver<AppCommand>,
) -> Result<Queue, RunnerError> {
    let are_paired = Arc::clone(&ctx.are_paired);
    let is_canceled = Arc::clone(&ctx.is_canceled);

    info!("Sending queue request to the server");
    ctx.log("Sending queue request to the server".to_string());

    let handle = spawn_queue_worker(ctx.clone(), ids, are_paired.clone());

    loop {
        if let Some(state) = check_cancel_cmd(ctx, cmd_rx) {
            is_canceled.store(true, Ordering::Relaxed);
            if ctx.change_state(state).is_err() {
                ctx.log("Error trying to execute cancel command".to_string());
            }

            ctx.client.send_cancel_queue()?;
            return Ok(Queue::Canceled);
        }

        if handle.is_finished() {
            if is_canceled.load(Ordering::Relaxed) {
                if ctx.change_state(ClientState::Idle).is_err() {
                    ctx.log("Error trying to go back to idle".to_string());
                }
                return Ok(Queue::Canceled);
            }
            return Ok(Queue::Matched);
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}

/// Tries to send queue request to the server and updates the client state
fn spawn_queue_worker(
    ctx: RunnerContext,
    ids: Vec<String>,
    are_paired: Arc<AtomicBool>,
) -> JoinHandle<()> {
    std::thread::spawn(move || {
        match ctx.client.send_queue_request(ids) {
            Ok(m_res) => {
                let session_id = m_res.session_id;
                let opp = m_res.opponent_discord_username;

                info!("Connected to {}", &opp);
                ctx.log(format!("Connected to {}", opp));

                let ok = ctx.change_state(ClientState::PlayingRanked(session_id));
                if ok.is_err() {
                    ctx.log("Internal error while starting ranked match".to_string());
                } else {
                    are_paired.store(true, Ordering::Relaxed);
                    return;
                }
            }
            Err(ClientError::ServerError(408)) => {
                error!("Queue request expired: {}", ClientError::ServerError(408));
                ctx.log("Queue request expired, could not pair players".to_string());
            }
            Err(ClientError::RequestError) => {
                error!("No response from the server: {}", ClientError::RequestError);
                ctx.log("No response from the server".to_string());
            }
            Err(_) if ctx.is_canceled() => {}
            Err(e) => {
                error!("Error trying to send queue request: {}", e);
                ctx.log("Error trying to send queue request, check logs for details".to_string());
            }
        }
        ctx.is_canceled.store(true, Ordering::Relaxed);
    })
}
