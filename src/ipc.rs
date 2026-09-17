//! Small session-local, bounded, nonblocking bar protocol (see docs/quickshell.md).
use crate::{Action, State};
use std::{
    io::{self, Read, Write},
    os::unix::{
        fs::PermissionsExt,
        net::{UnixListener, UnixStream},
    },
    path::PathBuf,
};
use wayland_client::Proxy;

struct Client {
    socket: UnixStream,
    input: Vec<u8>,
    output: Vec<u8>,
    snapshot: String,
}
pub struct Server {
    listener: UnixListener,
    path: PathBuf,
    clients: Vec<Client>,
}
impl Server {
    pub fn new() -> io::Result<Option<Self>> {
        let Some(path) = std::env::var_os("MYWM_SOCKET").map(PathBuf::from) else {
            return Ok(None);
        };
        // Never unlink another running session's socket. Stale sockets require cleanup.
        let listener = UnixListener::bind(&path)?;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        listener.set_nonblocking(true)?;
        Ok(Some(Self {
            listener,
            path,
            clients: Vec::new(),
        }))
    }
    pub fn update(&mut self, state: &mut State) {
        for _ in 0..16 {
            let Ok((socket, _)) = self.listener.accept() else {
                break;
            };
            if self.clients.len() >= 16 {
                continue;
            }
            if socket.set_nonblocking(true).is_ok() {
                self.clients.push(Client {
                    socket,
                    input: Vec::new(),
                    output: Vec::new(),
                    snapshot: String::new(),
                });
            }
        }
        let snapshot = snapshot(state);
        self.clients.retain_mut(|client| {
            let mut buf = [0; 1024];
            // Bound work per client, even when a peer continuously writes.
            for _ in 0..4 {
                match client.socket.read(&mut buf) {
                    Ok(0) => return false,
                    Ok(n) => client.input.extend_from_slice(&buf[..n]),
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                    Err(_) => return false,
                }
            }
            if client.input.len() > 4096 {
                return false;
            }
            while let Some(end) = client.input.iter().position(|c| *c == b'\n') {
                let line: Vec<_> = client.input.drain(..=end).collect();
                let action = std::str::from_utf8(&line)
                    .ok()
                    .and_then(|s| parse(s, state));
                let reply = if let Some(action) = action {
                    state.actions.push(action);
                    if let Some(manager) = &state.manager {
                        manager.manage_dirty();
                    }
                    "v1 ok\n"
                } else {
                    "v1 error invalid-command\n"
                };
                client.output.extend_from_slice(reply.as_bytes());
            }
            if client.snapshot != snapshot {
                client.output.extend_from_slice(snapshot.as_bytes());
                client.snapshot.clone_from(&snapshot);
            }
            if client.output.len() > 65536 {
                return false;
            }
            if !client.output.is_empty() {
                match client.socket.write(&client.output) {
                    Ok(0) => return false,
                    Ok(n) => {
                        client.output.drain(..n);
                    }
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
                    Err(_) => return false,
                }
            }
            true
        });
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}
fn parse(line: &str, state: &State) -> Option<Action> {
    let args: Vec<_> = line.split_whitespace().collect();
    if state.session_locked && args.as_slice() != ["v1", "lock"] {
        return None;
    }
    match args.as_slice() {
        ["v1", "lock"] => Some(Action::Lock),
        ["v1", "logout"] => Some(Action::Exit),
        ["v1", "workspace", output, workspace] => {
            let output = output.parse::<u32>().ok()?;
            let workspace = workspace.parse::<usize>().ok()?.checked_sub(1)?;
            (workspace < state.config.workspaces
                && state.outputs.iter().enumerate().any(|(index, o)| {
                    o.river_output.id().protocol_id() == output
                        && crate::monitor_workspaces::mask(state, index) & (1 << workspace) != 0
                }))
            .then_some(Action::WorkspaceOnOutput(output, workspace))
        }
        _ => None,
    }
}
fn snapshot(state: &State) -> String {
    let outputs: Vec<_> = state
        .outputs
        .iter()
        .enumerate()
        .filter_map(|(index, o)| {
            let (x, y) = o.position?;
            let (width, height) = o.dimensions?;
            let occupied = o
                .workspaces
                .entries
                .iter()
                .enumerate()
                .fold(0u32, |mask, (i, w)| {
                    mask | if w.windows.is_empty() { 0 } else { 1 << i }
                });
            Some(format!(
                "{},{},{},{},{},{},{},{}",
                o.river_output.id().protocol_id(),
                x,
                y,
                width,
                height,
                o.workspaces.active + 1,
                occupied,
                crate::monitor_workspaces::mask(state, index)
            ))
        })
        .collect();
    format!(
        "v1 state {} {}\nv1 locked {}\n",
        state.config.workspaces,
        outputs.join(";"),
        u8::from(state.session_locked)
    )
}
