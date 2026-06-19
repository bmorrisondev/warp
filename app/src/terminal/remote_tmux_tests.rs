use super::RemoteTmuxConnection;

#[test]
fn restore_ssh_command_returns_original_ssh_command() {
    let connection = RemoteTmuxConnection {
        ssh_command: "ssh user@example.com".to_string(),
        tmux_socket: "warp".to_string(),
        tmux_session_name: None,
    };

    assert_eq!(
        connection.restore_ssh_command(),
        "ssh user@example.com".to_string()
    );
}
