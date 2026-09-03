use std::collections::VecDeque;
use std::sync::Arc;

use tokio::net::TcpListener;
use tokio::task::{AbortHandle, JoinSet};

use super::google_oauth::{DesktopOAuthError, MAX_IN_FLIGHT_CALLBACKS};
use super::oauth_connection::{
    CallbackContext, ConnectionOutcome, TerminalCallback, handle_connection,
};

pub(super) async fn receive(
    listener: TcpListener,
    context: Arc<CallbackContext>,
) -> Result<TerminalCallback, DesktopOAuthError> {
    let mut connections = JoinSet::new();
    let mut active: VecDeque<(u64, AbortHandle)> = VecDeque::new();
    let mut next_connection_id = 0_u64;

    loop {
        let completed = tokio::select! {
            biased;
            completed = connections.join_next(), if !connections.is_empty() => completed,
            accepted = listener.accept() => {
                let (stream, _) = accepted
                    .map_err(|_| DesktopOAuthError::ListenerUnavailable)?;
                if active.len() == MAX_IN_FLIGHT_CALLBACKS {
                    let (oldest_id, oldest) = active
                        .pop_front()
                        .ok_or(DesktopOAuthError::ListenerUnavailable)?;
                    oldest.abort();
                    loop {
                        match connections.join_next().await {
                            Some(Ok((connection_id, outcome))) => {
                                remove_active(&mut active, connection_id);
                                if connection_id == oldest_id {
                                    if let ConnectionOutcome::Terminal(callback) = outcome {
                                        return Ok(callback);
                                    }
                                    break;
                                }
                                if let ConnectionOutcome::Terminal(callback) = outcome {
                                    return Ok(callback);
                                }
                            }
                            Some(Err(_)) => break,
                            None => return Err(DesktopOAuthError::ListenerUnavailable),
                        }
                    }
                }
                let connection_id = next_connection_id;
                next_connection_id = next_connection_id.wrapping_add(1);
                let handler_context = Arc::clone(&context);
                let abort_handle = connections.spawn(async move {
                    (connection_id, handle_connection(stream, handler_context).await)
                });
                active.push_back((connection_id, abort_handle));
                continue;
            }
        };

        match completed {
            Some(Ok((connection_id, outcome))) => {
                remove_active(&mut active, connection_id);
                match outcome {
                    ConnectionOutcome::Ignored => {}
                    ConnectionOutcome::Terminal(callback) => return Ok(callback),
                }
            }
            Some(Err(_)) | None => return Err(DesktopOAuthError::ListenerUnavailable),
        }
    }
}

fn remove_active(active: &mut VecDeque<(u64, AbortHandle)>, connection_id: u64) {
    if let Some(index) = active
        .iter()
        .position(|(active_id, _)| *active_id == connection_id)
    {
        active.remove(index);
    }
}
