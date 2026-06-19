use serde::{Deserialize, Serialize};

use crate::terminal::model::session::{Session, SessionType};
use crate::terminal::ssh::util::parse_interactive_ssh_command;
use crate::terminal::ShellLaunchData;
use warp_core::features::FeatureFlag;

/// Metadata for a Warp-managed remote tmux session that should survive window
/// restore and reconnect on startup.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteTmuxConnection {
    /// The interactive SSH command used to reach the remote host.
    pub ssh_command: String,
    /// The tmux socket name (Warp-managed sessions use the `warp` socket).
    #[serde(default = "default_tmux_socket")]
    pub tmux_socket: String,
    /// Optional tmux session name. When absent, `tmux attach` picks the default session.
    pub tmux_session_name: Option<String>,
}

fn default_tmux_socket() -> String {
    "warp".to_string()
}

impl RemoteTmuxConnection {
    /// Returns the SSH command to queue when restoring this connection.
    pub fn restore_ssh_command(&self) -> String {
        self.ssh_command.clone()
    }
}

/// Returns connection metadata for the active session when it is a warpified SSH
/// tmux session that should be restored on window reopen.
pub fn remote_tmux_connection_for_session(
    session: &Session,
    shell_launch_data: Option<&ShellLaunchData>,
) -> Option<RemoteTmuxConnection> {
    if !FeatureFlag::RemoteTmuxSessionRestore.is_enabled() {
        return None;
    }

    // Restored remote panes have no live shell launch data.
    if shell_launch_data.is_some() {
        return None;
    }

    if !session.info.tmux_control_mode {
        return None;
    }

    if !matches!(
        session.session_type(),
        SessionType::WarpifiedRemote { .. }
    ) {
        return None;
    }

    let subshell_info = session.subshell_info().as_ref()?;
    if subshell_info.ssh_connection_info.is_none()
        && parse_interactive_ssh_command(&subshell_info.spawning_command).is_none()
    {
        return None;
    }

    Some(RemoteTmuxConnection {
        ssh_command: subshell_info.spawning_command.clone(),
        tmux_socket: default_tmux_socket(),
        tmux_session_name: None,
    })
}

#[cfg(test)]
#[path = "remote_tmux_tests.rs"]
mod tests;
