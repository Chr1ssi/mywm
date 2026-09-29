mod actions;
mod appearance;
mod config;
mod drag;
mod events;
mod floating;
mod fullscreen;
mod ipc;
mod keyboard;
mod layer_shell;
mod layout;
mod monitor_workspaces;
mod outputs;
mod pointer;
mod river;
mod rules;
mod scrolling;
mod session;
mod shell;
mod theme;
mod vrr;
mod wallpaper;
mod windows;
mod workspaces;
use river::river_window_management::{
    river_node_v1::RiverNodeV1, river_output_v1::RiverOutputV1, river_seat_v1::RiverSeatV1,
    river_window_manager_v1::RiverWindowManagerV1, river_window_v1::RiverWindowV1,
};

use actions::*;
use config::Config;
use drag::*;
use floating::{DragKind, Rect};
use layer_shell::LayerFocus;
use layout::*;
use outputs::*;
use river::river_layer_shell::{
    river_layer_shell_output_v1::RiverLayerShellOutputV1,
    river_layer_shell_seat_v1::RiverLayerShellSeatV1, river_layer_shell_v1::RiverLayerShellV1,
};
use windows::*;

use river::river_window_management::{
    river_pointer_binding_v1::RiverPointerBindingV1, river_window_v1::Edges,
};

use river::river_xkb_bindings::river_xkb_binding_v1::RiverXkbBindingV1;
use river::river_xkb_bindings::river_xkb_bindings_seat_v1::RiverXkbBindingsSeatV1;
use river::river_xkb_bindings::river_xkb_bindings_v1::RiverXkbBindingsV1;
use wayland_client::backend::{ObjectId, WaylandError};
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle, protocol::wl_registry};
use workspaces::{GAMING, Workspace, Workspaces};

struct Window {
    river_window: RiverWindowV1,
    node: Option<RiverNodeV1>,
    geometry: Option<(i32, i32, i32, i32)>,
    actual_dimensions: Option<(i32, i32)>,
    proposed_dimensions: Option<(i32, i32)>,
    proposal_retried: bool,
    /// Propose one pixel less on the next layout to force a fresh configure.
    nudge: bool,
    /// The nudge was sent; propose the real size again on the next layout.
    nudge_restore: bool,
    output: Option<usize>,
    floating: bool,
    scratchpad_floating: Option<bool>,
    fullscreen: bool,
    fullscreen_output: Option<ObjectId>,
    floating_rect: Option<Rect>,
    tiled_width: Option<i32>,
    border_width: i32,
    app_id: Option<String>,
    parent: Option<ObjectId>,
}

struct Output {
    wl_global: Option<u32>,
    river_output: RiverOutputV1,
    position: Option<(i32, i32)>,
    dimensions: Option<(i32, i32)>,
    workspaces: Workspaces<ObjectId>,
    layer_output: Option<RiverLayerShellOutputV1>,
    non_exclusive_area: Option<Rect>,
    presentation_mode_set: bool,
}

struct State {
    wl_outputs: std::collections::HashMap<
        u32,
        (
            wayland_client::protocol::wl_output::WlOutput,
            Option<String>,
        ),
    >,
    windows: Vec<Window>,
    outputs: Vec<Output>,
    focused_output: Option<usize>,
    pointer_position: Option<(i32, i32)>,
    pointer_window: Option<ObjectId>,
    pointer_bindings: Vec<RiverPointerBindingV1>,
    pending_drag: Option<(ObjectId, PointerAction)>,
    drag: Option<Drag>,
    drag_delta: Option<(i32, i32)>,
    end_drag: bool,
    seat: Option<RiverSeatV1>,
    focused_window: Option<ObjectId>,
    focus_dirty: bool,
    session_locked: bool,
    locker: Option<std::process::Child>,
    manager: Option<RiverWindowManagerV1>,
    config: Config,
    keyboard: keyboard::KeyboardState,
    detached_workspaces: Option<Workspaces<ObjectId>>,
    scratchpad: Workspace<ObjectId>,
    scratchpad_visible: bool,

    bindings: Vec<RiverXkbBindingV1>,
    actions: Vec<Action>,
    exit_requested: bool,
    xkb_bindings: Option<RiverXkbBindingsV1>,
    layer_shell: Option<RiverLayerShellV1>,
    layer_seat: Option<RiverLayerShellSeatV1>,
    layer_focus: LayerFocus,
    layer_focus_granted: bool,
    vrr_enabled: Option<bool>,
    vrr_sender: Option<std::sync::mpsc::Sender<bool>>,
    dimension_retry_at: Option<std::time::Instant>,
}

#[derive(Clone, Copy, Debug)]
enum Action {
    Reload,
    Wallpaper,
    Lock,
    Terminal,
    Launcher,
    Exit,
    /// Output protocol id, workspace number (0 is the gaming workspace).
    WorkspaceOnOutput(u32, usize),
    Focus(isize),
    Move(isize),
    Close,
    ToggleFloating,
    ToggleScratchpad,
    MoveToScratchpad,
    FocusOutput(OutputDirection),
    MoveToOutput(OutputDirection),
    Workspace(usize),
    WorkspaceRelative(isize),
    MoveToWorkspace(usize),
    MoveToWorkspaceRelative(isize),
    NewWorkspace,
    MoveToNewWorkspace,
    Program(usize),
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum OutputDirection {
    Left,
    Right,
    Up,
    Down,
}

#[derive(Clone, Copy, Debug)]
enum PointerAction {
    Move,
    Resize,
    ResizeEdges(Edges),
}

struct Drag {
    window: ObjectId,
    initial: Rect,
    kind: DragKind,
    floating: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    eprintln!("mywm starting");
    let config = Config::load()?;

    match std::env::args().nth(1).as_deref() {
        Some("--theme-from-wallpaper") => {
            let wallpaper = std::env::args()
                .nth(2)
                .ok_or("--theme-from-wallpaper requires an image path")?;
            return theme::apply(std::path::Path::new(&wallpaper));
        }
        Some("--theme-from-state") => {
            let state = std::env::args()
                .nth(2)
                .ok_or("--theme-from-state requires a wallpaper state path")?;
            let directory = std::env::args()
                .nth(3)
                .unwrap_or_else(|| config.wallpaper_directory.clone());
            return theme::apply_from_wallpaper_state(
                std::path::Path::new(&state),
                std::path::Path::new(&directory),
            );
        }
        Some("--wallpaper-list") => return wallpaper::list(&config),
        Some("--wallpaper") => return wallpaper::run(&config),
        Some("--wallpaper-picker") => {
            let status = wallpaper::picker(0, 0).status()?;
            return status
                .success()
                .then_some(())
                .ok_or_else(|| "wallpaper picker exited unsuccessfully".into());
        }
        Some("--launcher") => {
            use std::os::unix::process::CommandExt;
            let mut command = std::process::Command::new(&config.launcher[0]);
            command.args(&config.launcher[1..]);
            config.apply_theme(&mut command);
            command.env("MYWM_TERMINAL_COUNT", config.terminal.len().to_string());
            for (index, argument) in config.terminal.iter().enumerate() {
                command.env(format!("MYWM_TERMINAL_{index}"), argument);
            }
            return Err(command.exec().into());
        }
        Some("--lock") => return session::lock_and_wait(&config),
        Some("--idle") => return session::idle(&config.idle),
        _ => {}
    }
    if std::env::args().nth(1).as_deref() == Some("--bar") {
        use std::os::unix::process::CommandExt;
        let mut command = std::process::Command::new("qs");
        command
            .arg("--path")
            .arg(shell::qml("bar.qml"))
            .arg("--no-duplicate");
        config.apply_theme(&mut command);
        return Err(command.exec().into());
    }
    let mut ipc = ipc::Server::new()?;
    let keyboard = keyboard::KeyboardState::new(&config.keyboard)?;
    let vrr_sender = vrr::worker(&config.vrr);
    let conn = Connection::connect_to_env()?;
    println!("connected to Wayland");

    let mut event_queue = conn.new_event_queue();
    let qh = event_queue.handle();

    let display = conn.display();
    display.get_registry(&qh, ());

    let mut state = State {
        wl_outputs: Default::default(),
        keyboard,
        manager: None,
        session_locked: false,
        locker: None,
        windows: Vec::new(),
        outputs: Vec::new(),
        focused_output: None,
        pointer_position: None,
        pointer_window: None,
        pointer_bindings: Vec::new(),
        pending_drag: None,
        drag: None,
        drag_delta: None,
        end_drag: false,
        seat: None,
        focused_window: None,
        focus_dirty: false,
        config,
        detached_workspaces: None,
        scratchpad: Workspace::default(),
        scratchpad_visible: false,
        bindings: Vec::new(),
        actions: Vec::new(),
        exit_requested: false,
        xkb_bindings: None,
        layer_shell: None,
        layer_seat: None,
        layer_focus: LayerFocus::None,
        layer_focus_granted: false,
        vrr_enabled: None,
        vrr_sender,
        dimension_retry_at: None,
    };

    event_queue.roundtrip(&mut state)?;

    println!("Entering event loop");

    loop {
        let result = (|| -> Result<(), Box<dyn std::error::Error>> {
            use std::os::fd::AsRawFd;
            event_queue.dispatch_pending(&mut state)?;
            if let Some(ipc) = &mut ipc {
                ipc.update(&mut state);
            }
            run_dimension_retry(&mut state);
            let flush_blocked = match conn.flush() {
                Ok(()) => false,
                Err(WaylandError::Io(error)) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    true
                }
                Err(error) => return Err(error.into()),
            };
            if let Some(guard) = event_queue.prepare_read() {
                let mut fd = libc::pollfd {
                    fd: guard.connection_fd().as_raw_fd(),
                    events: libc::POLLIN | if flush_blocked { libc::POLLOUT } else { 0 },
                    revents: 0,
                };
                // A bounded wait services bar clients and scheduled layout retries while idle.
                let ipc_timeout = ipc.as_ref().map(|_| 100);
                let retry_timeout = state.dimension_retry_at.map(|at| {
                    at.saturating_duration_since(std::time::Instant::now())
                        .as_millis()
                        .min(i32::MAX as u128) as i32
                });
                let timeout = match (ipc_timeout, retry_timeout) {
                    (Some(a), Some(b)) => a.min(b),
                    (Some(value), None) | (None, Some(value)) => value,
                    (None, None) => -1,
                };
                let ready = unsafe { libc::poll(&mut fd, 1, timeout) };
                if ready < 0 {
                    let error = std::io::Error::last_os_error();
                    if error.kind() != std::io::ErrorKind::Interrupted {
                        return Err(error.into());
                    }
                } else if ready > 0 && fd.revents & libc::POLLIN != 0 {
                    match guard.read() {
                        Ok(_) => {}
                        Err(WaylandError::Io(error))
                            if error.kind() == std::io::ErrorKind::WouldBlock => {}
                        Err(error) => return Err(error.into()),
                    }
                }
            }
            Ok(())
        })();
        if let Err(error) = result {
            if state.exit_requested {
                return Ok(());
            }
            return Err(error);
        }
    }
}
