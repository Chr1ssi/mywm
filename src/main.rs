mod appearance;
mod config;
mod floating;
mod fullscreen;
mod ipc;
mod keyboard;
mod layer_shell;
mod monitor_workspaces;
mod pointer;
mod river;
mod rules;
mod scrolling;
mod session;
mod shell;
mod theme;
mod vrr;
mod wallpaper;
mod workspaces;

use river::river_window_management::{
    river_node_v1::RiverNodeV1, river_output_v1::RiverOutputV1, river_seat_v1::RiverSeatV1,
    river_window_manager_v1::RiverWindowManagerV1, river_window_v1::RiverWindowV1,
};

use config::Config;
use floating::{DragKind, Rect};
use layer_shell::LayerFocus;
use river::river_layer_shell::{
    river_layer_shell_output_v1::RiverLayerShellOutputV1,
    river_layer_shell_seat_v1::RiverLayerShellSeatV1, river_layer_shell_v1::RiverLayerShellV1,
};
use river::river_window_management::{
    river_pointer_binding_v1::RiverPointerBindingV1, river_window_v1::Edges,
};
use river::river_xkb_bindings::river_xkb_binding_v1::RiverXkbBindingV1;
use river::river_xkb_bindings::river_xkb_bindings_seat_v1::RiverXkbBindingsSeatV1;
use river::river_xkb_bindings::river_xkb_bindings_v1::RiverXkbBindingsV1;
use wayland_client::backend::{ObjectId, WaylandError};
use workspaces::{Workspace, Workspaces};

use wayland_client::{Connection, Dispatch, Proxy, QueueHandle, protocol::wl_registry};

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

fn relative_workspace(state: &State, output: usize, direction: isize) -> Option<usize> {
    let allowed = monitor_workspaces::mask(state, output);
    let workspaces: Vec<_> = (0..state.config.workspaces)
        .filter(|workspace| allowed & (1 << workspace) != 0)
        .collect();
    let current = state.outputs[output].workspaces.active;
    let position = workspaces
        .iter()
        .position(|workspace| *workspace == current)?;
    Some(workspaces[(position as isize + direction).rem_euclid(workspaces.len() as isize) as usize])
}

fn is_game_window(state: &State, id: &ObjectId) -> bool {
    let mut current = Some(id.clone());
    for _ in 0..=state.windows.len() {
        let Some(window) = current.as_ref().and_then(|id| {
            state
                .windows
                .iter()
                .find(|window| window.river_window.id() == *id)
        }) else {
            return false;
        };
        if window.app_id.as_deref().is_some_and(|app_id| {
            state
                .config
                .game_app_id_prefixes
                .iter()
                .any(|prefix| app_id.starts_with(prefix))
        }) {
            return true;
        }
        current = window.parent.clone();
    }
    false
}

fn needs_dimension_retry(window: &Window) -> bool {
    window.fullscreen_output.is_none()
        && window.actual_dimensions.is_some()
        && window.actual_dimensions != window.proposed_dimensions
        && !window.proposal_retried
}

fn run_dimension_retry(state: &mut State) {
    let Some(deadline) = state.dimension_retry_at else {
        return;
    };
    if std::time::Instant::now() < deadline {
        return;
    }
    state.dimension_retry_at = None;
    let mut retry_layout = false;
    for window in &mut state.windows {
        if window.nudge_restore {
            window.nudge_restore = false;
            retry_layout = true;
        } else if needs_dimension_retry(window) {
            // Resending the same proposal does not produce a new configure, and
            // Chromium keeps a stale window geometry (and thus a wrongly sized
            // border) after the output changed. A differing size makes it commit
            // fresh geometry.
            window.proposal_retried = true;
            window.nudge = true;
            retry_layout = true;
        }
    }
    if retry_layout && let Some(manager) = &state.manager {
        manager.manage_dirty();
    }
}

fn workspace_accepts(state: &State, workspace: usize, id: &ObjectId) -> bool {
    state.config.gaming_workspace.map(|number| number - 1) != Some(workspace)
        || is_game_window(state, id)
}

fn cancel_drag(state: &mut State) {
    state.pending_drag = None;
    state.end_drag = true;
}

fn update_drag(state: &mut State) {
    if let Some(drag) = &state.drag {
        if let Some(window) = state
            .windows
            .iter_mut()
            .find(|w| w.river_window.id() == drag.window)
            && let Some(output) = window.output.and_then(|i| state.outputs.get(i))
            && (output.workspaces.current().windows.contains(&drag.window)
                || (state.scratchpad_visible && state.scratchpad.windows.contains(&drag.window)))
            && let Some(Rect { width, height, .. }) = output.work_area()
        {
            if let Some((dx, dy)) = state.drag_delta.take() {
                if drag.floating {
                    window.floating_rect =
                        Some(drag.initial.dragged(drag.kind, dx, dy, width, height));
                } else if let DragKind::Resize(edges) = drag.kind {
                    let delta = if edges.contains(Edges::Left) { -dx } else { dx };
                    window.tiled_width = Some(
                        drag.initial
                            .width
                            .saturating_add(delta)
                            .clamp((width / 4).max(100).min(width), width.max(1)),
                    );
                }
            }
        } else {
            state.end_drag = true;
        }
    }
    if state.end_drag {
        if let Some(drag) = state.drag.take() {
            if let Some(seat) = &state.seat {
                seat.op_end();
            }
            if matches!(drag.kind, DragKind::Resize(_))
                && let Some(window) = state
                    .windows
                    .iter()
                    .find(|w| w.river_window.id() == drag.window)
            {
                window.river_window.inform_resize_end();
            }
        }
        state.drag_delta = None;
        state.end_drag = false;
    }
    let Some((id, action)) = state.pending_drag.take() else {
        return;
    };
    if state.drag.is_some()
        || state.seat.is_none()
        || state.layer_focus == LayerFocus::Exclusive
        || state.layer_focus_granted
    {
        return;
    }
    let Some(index) = state
        .windows
        .iter()
        .position(|w| w.river_window.id() == id && !w.fullscreen)
    else {
        return;
    };
    let Some(output_index) = state.windows[index].output else {
        return;
    };
    let output = &state.outputs[output_index];
    if !output.workspaces.current().windows.contains(&id)
        && !(state.scratchpad_visible && state.scratchpad.windows.contains(&id))
    {
        return;
    }
    let Some(Rect {
        x: ox,
        y: oy,
        width,
        height,
    }) = output.work_area()
    else {
        return;
    };
    let floating = state.windows[index].floating;
    if !floating && matches!(action, PointerAction::Move) {
        return;
    }
    let initial = if floating {
        state.windows[index]
            .floating_rect
            .unwrap_or_else(|| Rect::centered(width, height))
            .constrained(width, height)
    } else {
        let current_width = state.windows[index]
            .geometry
            .map(|geometry| geometry.2 + state.windows[index].border_width * 2)
            .unwrap_or(width / 2);
        Rect {
            x: 0,
            y: 0,
            width: current_width,
            height,
        }
    };
    let kind = match action {
        PointerAction::Move => DragKind::Move,
        PointerAction::ResizeEdges(edges) => DragKind::Resize(edges),
        PointerAction::Resize if floating => {
            let (x, y) = state.pointer_position.unwrap_or((
                ox + initial.x + initial.width,
                oy + initial.y + initial.height,
            ));
            let horizontal = if x - ox < initial.x + initial.width / 2 {
                Edges::Left
            } else {
                Edges::Right
            };
            let vertical = if y - oy < initial.y + initial.height / 2 {
                Edges::Top
            } else {
                Edges::Bottom
            };
            DragKind::Resize(horizontal | vertical)
        }
        PointerAction::Resize => DragKind::Resize(Edges::Right),
    };
    state.outputs[output_index].workspaces.focus(&id);
    focus_output(state, output_index);
    state.seat.as_ref().unwrap().op_start_pointer();
    if matches!(kind, DragKind::Resize(_)) {
        state
            .windows
            .iter()
            .find(|w| w.river_window.id() == id)
            .unwrap()
            .river_window
            .inform_resize_start();
    }
    state.drag = Some(Drag {
        window: id,
        initial,
        kind,
        floating,
    });
}

fn assign_windows(state: &mut State, default_output: usize) {
    let mut focus = None;
    // A child may be announced before its parent. Assign parents first so dialog
    // placement also inherits any workspace rule applied to the parent.
    loop {
        let mut assigned = false;
        for index in 0..state.windows.len() {
            if state.windows[index].output.is_some() {
                continue;
            }
            let window = &state.windows[index];
            let parent = window
                .parent
                .as_ref()
                .and_then(|id| state.windows.iter().find(|w| w.river_window.id() == *id));
            let inherited = if let Some(parent) = parent {
                let Some(output) = parent.output else {
                    continue;
                };
                let workspace = state.outputs[output]
                    .workspaces
                    .location(&parent.river_window.id())
                    .unwrap_or(state.outputs[output].workspaces.active);
                Some((output, workspace))
            } else {
                None
            };
            let placement = rules::resolve(
                &state.config.rules,
                window.app_id.as_deref(),
                window.parent.is_some(),
                state.config.float_dialogs,
            );
            let (output, workspace) = inherited.unwrap_or((
                default_output,
                state.outputs[default_output].workspaces.active,
            ));
            let id = window.river_window.id();
            let mut workspace = placement.workspace.unwrap_or(workspace);
            let gaming = state.config.gaming_workspace.map(|number| number - 1);
            let game = is_game_window(state, &id);
            if game {
                if let Some(gaming) = gaming {
                    workspace = gaming;
                }
            } else if gaming == Some(workspace) {
                workspace = (0..state.config.workspaces)
                    .find(|candidate| {
                        Some(*candidate) != gaming
                            && monitor_workspaces::owner(state, *candidate)
                                .is_none_or(|owner| owner == output)
                    })
                    .unwrap_or(workspace);
                state.outputs[output].workspaces.select(workspace);
            }
            let output = monitor_workspaces::owner(state, workspace).unwrap_or(output);
            if game {
                state.outputs[output].workspaces.select(workspace);
            }
            state.windows[index].output = Some(output);
            state.windows[index].floating = placement.floating;
            if workspace == state.outputs[output].workspaces.active {
                state.outputs[output].workspaces.add(id);
                focus = Some(output);
            } else {
                state.outputs[output].workspaces.add_to(workspace, id);
            }
            assigned = true;
        }
        if !assigned {
            break;
        }
    }
    if let Some(output) = focus {
        focus_output(state, output);
    }
}

fn focus_output(state: &mut State, output: usize) {
    state.focused_output = Some(output);
    state.focused_window = state.outputs[output].workspaces.current().focused.clone();
    state.focus_dirty = true;
    // Global storage order doubles as floating stacking order, independent of columns.
    if let Some(index) = state
        .windows
        .iter()
        .position(|w| w.floating && Some(w.river_window.id()) == state.focused_window)
    {
        let window = state.windows.remove(index);
        state.windows.push(window);
    }
}

fn focus_scratchpad(state: &mut State) {
    state.focused_window = state.scratchpad.focused.clone();
    state.focus_dirty = true;
}

fn adjacent_output(state: &State, source: usize, direction: OutputDirection) -> Option<usize> {
    let source_area = state.outputs.get(source)?.work_area()?;
    let source_center = (
        source_area.x + source_area.width / 2,
        source_area.y + source_area.height / 2,
    );
    state
        .outputs
        .iter()
        .enumerate()
        .filter_map(|(index, output)| {
            if index == source {
                return None;
            }
            let area = output.work_area()?;
            let dx = area.x + area.width / 2 - source_center.0;
            let dy = area.y + area.height / 2 - source_center.1;
            let (forward, sideways) = match direction {
                OutputDirection::Left if dx < 0 => (-dx, dy.abs()),
                OutputDirection::Right if dx > 0 => (dx, dy.abs()),
                OutputDirection::Up if dy < 0 => (-dy, dx.abs()),
                OutputDirection::Down if dy > 0 => (dy, dx.abs()),
                _ => return None,
            };
            Some((
                sideways as i64 * sideways as i64 + forward as i64 * forward as i64,
                index,
            ))
        })
        .min_by_key(|candidate| candidate.0)
        .map(|candidate| candidate.1)
}

fn warp_to_output(state: &mut State, output: usize) {
    if let Some(area) = state.outputs[output].work_area()
        && let Some(seat) = &state.seat
        && seat.version() >= 3
    {
        let position = (area.x + area.width / 2, area.y + area.height / 2);
        seat.pointer_warp(position.0, position.1);
        state.pointer_position = Some(position);
    }
}

fn layout(state: &mut State) {
    for window in &mut state.windows {
        window.geometry = None;
    }
    for output in &mut state.outputs {
        let Some(Rect {
            x,
            y,
            width,
            height,
        }) = output.work_area()
        else {
            continue;
        };
        let workspace = output.workspaces.current_mut();
        let tiled: Vec<_> = workspace
            .windows
            .iter()
            .filter(|id| {
                state
                    .windows
                    .iter()
                    .any(|w| w.river_window.id() == **id && !w.floating)
            })
            .collect();
        // Keep a focused dialog's tiled ancestor visible behind it.
        let mut scroll_focus = workspace.focused.clone();
        let mut focused = None;
        for _ in 0..=state.windows.len() {
            focused = tiled
                .iter()
                .position(|id| Some(*id) == scroll_focus.as_ref());
            if focused.is_some() || scroll_focus.is_none() {
                break;
            }
            scroll_focus = state
                .windows
                .iter()
                .find(|w| Some(w.river_window.id()) == scroll_focus)
                .and_then(|w| w.parent.clone());
        }
        let viewport = state.config.appearance.viewport(Rect {
            x,
            y,
            width,
            height,
        });
        let widths: Vec<_> = tiled
            .iter()
            .map(|id| {
                state
                    .windows
                    .iter()
                    .find(|window| window.river_window.id() == **id)
                    .and_then(|window| window.tiled_width)
            })
            .collect();
        let (scroll, columns) = scrolling::variable_columns(
            viewport.width,
            &widths,
            focused,
            workspace.scroll,
            state.config.appearance.gaps_inner,
        );
        workspace.scroll = scroll;
        for (id, (left, column_width)) in tiled.into_iter().zip(columns) {
            if let Some(window) = state
                .windows
                .iter_mut()
                .find(|w| w.river_window.id() == *id)
            {
                let (content, border) = state.config.appearance.content(Rect {
                    x: viewport.x + left,
                    y: viewport.y,
                    width: column_width,
                    height: viewport.height,
                });
                window.geometry = Some((content.x, content.y, content.width, content.height));
                window.border_width = border;
            }
        }
        for window in state
            .windows
            .iter_mut()
            .filter(|w| w.floating && workspace.windows.contains(&w.river_window.id()))
        {
            let rect = window
                .floating_rect
                .unwrap_or_else(|| Rect::centered(width, height))
                .constrained(width, height);
            window.floating_rect = Some(rect);
            let (content, border) = state.config.appearance.content(Rect {
                x: x + rect.x,
                y: y + rect.y,
                width: rect.width,
                height: rect.height,
            });
            window.geometry = Some((content.x, content.y, content.width, content.height));
            window.border_width = border;
        }
    }
    if state.scratchpad_visible
        && let Some(output) = output_at_pointer(state).or(state.focused_output)
        && let Some(Rect {
            x,
            y,
            width,
            height,
        }) = state.outputs[output].work_area()
    {
        for id in state.scratchpad.windows.clone() {
            if let Some(window) = state.windows.iter_mut().find(|w| w.river_window.id() == id) {
                window.output = Some(output);
                window.floating = true;
                let rect = window
                    .floating_rect
                    .unwrap_or_else(|| Rect::centered(width, height))
                    .constrained(width, height);
                window.floating_rect = Some(rect);
                let (content, border) = state.config.appearance.content(Rect {
                    x: x + rect.x,
                    y: y + rect.y,
                    width: rect.width,
                    height: rect.height,
                });
                window.geometry = Some((content.x, content.y, content.width, content.height));
                window.border_width = border;
            }
        }
    }
}

fn run_actions(state: &mut State, manager: &RiverWindowManagerV1, qh: &QueueHandle<State>) {
    for action in std::mem::take(&mut state.actions) {
        if state.session_locked && !matches!(action, Action::Lock) {
            continue;
        }
        match action {
            Action::Reload => reload_config(state, qh),
            Action::Wallpaper => {
                let (x, y) = state
                    .pointer_position
                    .or_else(|| state.focused_output.and_then(|i| state.outputs[i].position))
                    .unwrap_or((0, 0));
                match wallpaper::picker(x, y).spawn() {
                    Ok(mut child) => {
                        std::thread::spawn(move || {
                            let _ = child.wait();
                        });
                    }
                    Err(error) => eprintln!("Cannot open wallpaper picker: {error}"),
                }
            }
            Action::Lock => {
                session::start_lock(&state.config, &mut state.locker, state.session_locked)
            }
            Action::Exit => {
                if manager.version() >= 4 {
                    state.exit_requested = true;
                    manager.exit_session();
                    return;
                }
                eprintln!(
                    "Exiting the session requires river-window-management-v1 version 4 or newer"
                );
            }
            Action::Terminal => {
                match std::process::Command::new(&state.config.terminal[0])
                    .args(&state.config.terminal[1..])
                    .spawn()
                {
                    Ok(mut child) => {
                        std::thread::spawn(move || {
                            let _ = child.wait();
                        });
                    }
                    Err(error) => eprintln!("Cannot start terminal: {error}"),
                }
            }
            Action::Launcher => {
                let mut command = std::process::Command::new(&state.config.launcher[0]);
                command.args(&state.config.launcher[1..]);
                state.config.apply_theme(&mut command);
                command.env(
                    "MYWM_TERMINAL_COUNT",
                    state.config.terminal.len().to_string(),
                );
                for (index, argument) in state.config.terminal.iter().enumerate() {
                    command.env(format!("MYWM_TERMINAL_{index}"), argument);
                }
                if let Some((x, y)) = state.pointer_position.or_else(|| {
                    state
                        .focused_output
                        .and_then(|i| state.outputs[i].work_area())
                        .map(|r| (r.x + r.width / 2, r.y + r.height / 2))
                }) {
                    command
                        .env("MYWM_LAUNCHER_X", x.to_string())
                        .env("MYWM_LAUNCHER_Y", y.to_string());
                }
                match command.spawn() {
                    Ok(mut child) => {
                        std::thread::spawn(move || {
                            let _ = child.wait();
                        });
                    }
                    Err(error) => eprintln!("Cannot start launcher: {error}"),
                }
            }
            Action::Program(index) => {
                if let Some(binding) = state.config.program_bindings.values().nth(index) {
                    spawn_command(&binding.command, "program binding");
                }
            }
            Action::WorkspaceOnOutput(id, target) => {
                if let Some(output) = state
                    .outputs
                    .iter()
                    .position(|o| o.river_output.id().protocol_id() == id)
                {
                    if monitor_workspaces::owner(state, target).is_some_and(|owner| owner != output)
                    {
                        continue;
                    }
                    cancel_drag(state);
                    monitor_workspaces::select(state, output, target);
                }
            }
            Action::Workspace(target) => {
                cancel_drag(state);
                if let Some(output) = monitor_workspaces::owner(state, target)
                    .or_else(|| output_at_pointer(state).or(state.focused_output))
                {
                    monitor_workspaces::select(state, output, target);
                }
            }
            Action::WorkspaceRelative(direction) => {
                cancel_drag(state);
                if let Some(output) = output_at_pointer(state).or(state.focused_output)
                    && let Some(target) = relative_workspace(state, output, direction)
                {
                    monitor_workspaces::select(state, output, target);
                }
            }
            Action::ToggleFloating => {
                cancel_drag(state);
                if let Some(window) = state
                    .windows
                    .iter_mut()
                    .find(|w| Some(w.river_window.id()) == state.focused_window)
                {
                    window.floating = !window.floating;
                    if let Some(output) = window.output {
                        focus_output(state, output);
                    }
                }
            }
            Action::ToggleScratchpad => {
                if state.scratchpad.windows.is_empty() {
                    continue;
                }
                cancel_drag(state);
                state.scratchpad_visible = !state.scratchpad_visible;
                if state.scratchpad_visible {
                    focus_scratchpad(state);
                } else if let Some(output) = output_at_pointer(state).or(state.focused_output) {
                    focus_output(state, output);
                }
            }
            Action::MoveToScratchpad => {
                let Some(id) = state.focused_window.clone() else {
                    continue;
                };
                cancel_drag(state);
                if state.scratchpad.windows.contains(&id) {
                    state.scratchpad.remove(&id);
                    state.scratchpad_visible = !state.scratchpad.windows.is_empty();
                    if let Some(output) = output_at_pointer(state).or(state.focused_output) {
                        state.outputs[output].workspaces.add(id.clone());
                        if let Some(window) =
                            state.windows.iter_mut().find(|w| w.river_window.id() == id)
                        {
                            window.output = Some(output);
                            window.floating = window.scratchpad_floating.take().unwrap_or(true);
                        }
                        focus_output(state, output);
                    }
                } else {
                    for output in &mut state.outputs {
                        output.workspaces.remove(&id);
                    }
                    state.scratchpad.windows.push(id.clone());
                    state.scratchpad.focused = Some(id.clone());
                    state.scratchpad_visible = true;
                    if let Some(window) =
                        state.windows.iter_mut().find(|w| w.river_window.id() == id)
                    {
                        window.scratchpad_floating = Some(window.floating);
                        window.floating = true;
                    }
                    focus_scratchpad(state);
                }
            }
            Action::FocusOutput(direction) => {
                if let Some(source) = output_at_pointer(state).or(state.focused_output)
                    && let Some(target) = adjacent_output(state, source, direction)
                {
                    focus_output(state, target);
                    warp_to_output(state, target);
                }
            }
            Action::MoveToOutput(direction) => {
                let Some(id) = state.focused_window.clone() else {
                    continue;
                };
                let Some(source) = state
                    .windows
                    .iter()
                    .find(|window| window.river_window.id() == id)
                    .and_then(|window| window.output)
                else {
                    continue;
                };
                let horizontal = match direction {
                    OutputDirection::Left => Some(-1),
                    OutputDirection::Right => Some(1),
                    OutputDirection::Up | OutputDirection::Down => None,
                };
                if !state
                    .windows
                    .iter()
                    .any(|window| window.river_window.id() == id && window.floating)
                    && let Some(direction) = horizontal
                {
                    let tiled: Vec<_> = state
                        .windows
                        .iter()
                        .filter(|window| !window.floating)
                        .map(|window| window.river_window.id())
                        .collect();
                    if state.outputs[source]
                        .workspaces
                        .can_navigate_matching(direction, |candidate| tiled.contains(candidate))
                    {
                        state.outputs[source].workspaces.navigate_matching(
                            direction,
                            true,
                            |candidate| tiled.contains(candidate),
                        );
                        focus_output(state, source);
                        continue;
                    }
                }
                let Some(target) = adjacent_output(state, source, direction) else {
                    continue;
                };
                let workspace = state.outputs[target].workspaces.active;
                if !workspace_accepts(state, workspace, &id) {
                    continue;
                }
                cancel_drag(state);
                state.outputs[source].workspaces.remove(&id);
                state.outputs[target]
                    .workspaces
                    .add_to(workspace, id.clone());
                if let Some(window) = state
                    .windows
                    .iter_mut()
                    .find(|window| window.river_window.id() == id)
                {
                    window.output = Some(target);
                    if window.floating {
                        window.floating_rect = None;
                    }
                }
                focus_output(state, target);
                warp_to_output(state, target);
            }
            Action::Close
            | Action::Focus(_)
            | Action::Move(_)
            | Action::MoveToWorkspace(_)
            | Action::MoveToWorkspaceRelative(_) => {
                let Some((window_id, output, floating)) = state
                    .windows
                    .iter()
                    .find(|w| Some(w.river_window.id()) == state.focused_window)
                    .and_then(|window| {
                        Some((window.river_window.id(), window.output?, window.floating))
                    })
                else {
                    continue;
                };
                match action {
                    Action::Close => {
                        if let Some(window) = state
                            .windows
                            .iter()
                            .find(|window| window.river_window.id() == window_id)
                        {
                            window.river_window.close();
                        }
                    }
                    Action::Focus(direction) | Action::Move(direction) => {
                        if floating && matches!(action, Action::Move(_)) {
                            continue;
                        }
                        if matches!(action, Action::Move(_)) {
                            let tiled: Vec<_> = state
                                .windows
                                .iter()
                                .filter(|w| !w.floating)
                                .map(|w| w.river_window.id())
                                .collect();
                            state.outputs[output].workspaces.navigate_matching(
                                direction,
                                true,
                                |id| tiled.contains(id),
                            );
                        } else {
                            state.outputs[output].workspaces.navigate(direction, false);
                        }
                        focus_output(state, output);
                    }
                    Action::MoveToWorkspace(target) => {
                        cancel_drag(state);
                        if workspace_accepts(state, target, &window_id) {
                            monitor_workspaces::move_window(state, output, target);
                        }
                    }
                    Action::MoveToWorkspaceRelative(direction) => {
                        cancel_drag(state);
                        if let Some(target) = relative_workspace(state, output, direction)
                            && workspace_accepts(state, target, &window_id)
                        {
                            monitor_workspaces::move_window(state, output, target);
                        }
                    }
                    _ => unreachable!(),
                }
            }
        }
    }
}

fn spawn_command(command: &[String], context: &str) {
    match std::process::Command::new(&command[0])
        .args(&command[1..])
        .spawn()
    {
        Ok(mut child) => {
            std::thread::spawn(move || {
                let _ = child.wait();
            });
        }
        Err(error) => eprintln!("Cannot start {context} '{}': {error}", command[0]),
    }
}

fn reload_config(state: &mut State, qh: &QueueHandle<State>) {
    let new = match Config::load() {
        Ok(config) => config,
        Err(error) => {
            eprintln!("Configuration reload failed; keeping current configuration: {error}");
            return;
        }
    };

    vrr::disable(state);
    for binding in state.bindings.drain(..) {
        binding.destroy();
    }
    for binding in state.pointer_bindings.drain(..) {
        binding.destroy();
    }
    state.config.apply_reloadable(new);
    state.vrr_sender = vrr::worker(&state.config.vrr);
    install_bindings(state, qh);
    state.focus_dirty = true;
    eprintln!("Configuration reloaded");
}

fn install_bindings(state: &mut State, qh: &QueueHandle<State>) {
    let Some(seat) = &state.seat else {
        return;
    };
    if state.pointer_bindings.is_empty() {
        let modifiers = state
            .config
            .pointer_modifiers()
            .expect("validated pointer modifiers");
        for (button, action) in [(0x110, PointerAction::Move), (0x111, PointerAction::Resize)] {
            let binding = seat.get_pointer_binding(button, modifiers, qh, action);
            binding.enable();
            state.pointer_bindings.push(binding);
        }
    }
    if !state.bindings.is_empty() {
        return;
    }
    let Some(bindings) = &state.xkb_bindings else {
        return;
    };
    for (key, modifiers, action) in state.config.keybindings().expect("validated configuration") {
        let binding = bindings.get_xkb_binding(seat, key, modifiers, qh, action);
        binding.enable();
        state.bindings.push(binding);
    }
}

fn update_focused_output(state: &mut State, pointer_x: i32, pointer_y: i32) {
    let new_focused_output = state.outputs.iter().position(|output| {
        let Some((output_x, output_y)) = output.position else {
            return false;
        };

        let Some((output_width, output_height)) = output.dimensions else {
            return false;
        };

        pointer_x >= output_x
            && pointer_x < output_x + output_width
            && pointer_y >= output_y
            && pointer_y < output_y + output_height
    });

    if state.focused_output != new_focused_output {
        state.focused_output = new_focused_output;

        if let Some(index) = new_focused_output {
            println!(
                "Focused output changed: {} (pointer: {}, {})",
                index, pointer_x, pointer_y
            );
        }
    }
}

fn output_at_pointer(state: &State) -> Option<usize> {
    let (pointer_x, pointer_y) = state.pointer_position?;

    state.outputs.iter().position(|output| {
        let Some((output_x, output_y)) = output.position else {
            return false;
        };

        let Some((output_width, output_height)) = output.dimensions else {
            return false;
        };

        pointer_x >= output_x
            && pointer_x < output_x + output_width
            && pointer_y >= output_y
            && pointer_y < output_y + output_height
    })
}

fn remove_window(state: &mut State, window_id: wayland_client::backend::ObjectId) {
    let Some(closed_index) = state
        .windows
        .iter()
        .position(|window| window.river_window.id() == window_id)
    else {
        return;
    };

    if state.pointer_window.as_ref() == Some(&window_id) {
        state.pointer_window = None;
    }
    if state.drag.as_ref().is_some_and(|d| d.window == window_id) {
        cancel_drag(state);
    }
    let removed = state.windows.remove(closed_index);
    if let Some(node) = removed.node {
        node.destroy();
    }
    removed.river_window.destroy();
    for output in &mut state.outputs {
        output.workspaces.remove(&window_id);
    }
    state.scratchpad.remove(&window_id);
    if let Some(workspaces) = &mut state.detached_workspaces {
        workspaces.remove(&window_id);
    }
    if state.focused_window.as_ref() == Some(&window_id) {
        if let Some(output) = removed.output {
            focus_output(state, output);
        } else {
            state.focused_window = None;
            state.focus_dirty = true;
        }
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for State {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _data: &(),
        _conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::GlobalRemove { name } = &event
            && let Some((output, _)) = state.wl_outputs.remove(name)
            && output.version() >= 3
        {
            output.release();
        }
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            keyboard::bind(state, registry, name, &interface, version, qh);
            if interface == "river_libinput_config_v1" {
                registry.bind::<river::river_libinput_config::river_libinput_config_v1::RiverLibinputConfigV1, _, _>(
                    name, version.min(2), qh, (),
                );
            }
            if interface == "wl_output" {
                let output = registry.bind::<wayland_client::protocol::wl_output::WlOutput, _, _>(
                    name,
                    version.min(4),
                    qh,
                    name,
                );
                state.wl_outputs.insert(name, (output, None));
            }
            if interface == "river_window_manager_v1" {
                println!("Binding river_window_manager_v1 v{version}");

                state.manager =
                    Some(registry.bind::<RiverWindowManagerV1, _, _>(name, version.min(5), qh, ()));
            }

            if interface == "river_layer_shell_v1" {
                state.layer_shell = Some(registry.bind::<RiverLayerShellV1, _, _>(name, 1, qh, ()));
                layer_shell::setup(state, qh);
            }

            if interface == "river_xkb_bindings_v1" {
                println!("Binding river_xkb_bindings_v1 v{version}");

                let bindings =
                    registry.bind::<RiverXkbBindingsV1, _, _>(name, version.min(3), qh, ());

                state.xkb_bindings = Some(bindings);
            }
        }
    }
}

impl Dispatch<RiverWindowManagerV1, ()> for State {
    fn event(
        state: &mut Self,
        manager: &RiverWindowManagerV1,
        event: river::river_window_management::river_window_manager_v1::Event,
        _data: &(),
        _conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        match event {
            river::river_window_management::river_window_manager_v1::Event::Window { id } => {
                state.windows.push(Window {
                    river_window: id,
                    node: None,
                    geometry: None,
                    actual_dimensions: None,
                    proposed_dimensions: None,
                    proposal_retried: false,
                    nudge: false,
                    nudge_restore: false,
                    output: None,
                    floating: false,
                    scratchpad_floating: None,
                    fullscreen: false,
                    fullscreen_output: None,
                    floating_rect: None,
                    tiled_width: None,
                    border_width: 0,
                    app_id: None,
                    parent: None,
                });
            }

            river::river_window_management::river_window_manager_v1::Event::Output { id } => {
                state.outputs.push(Output {
                    wl_global: None,
                    river_output: id,
                    position: None,
                    dimensions: None,
                    workspaces: Workspaces::new(state.config.workspaces),
                    layer_output: None,
                    non_exclusive_area: None,
                    presentation_mode_set: false,
                });

                layer_shell::setup(state, qh);
                if let Some(workspaces) = state.detached_workspaces.take() {
                    let output_index = state.outputs.len() - 1;
                    for window in &mut state.windows {
                        if workspaces
                            .entries
                            .iter()
                            .any(|ws| ws.windows.contains(&window.river_window.id()))
                        {
                            window.output = Some(output_index);
                        }
                    }
                    state.outputs[output_index].workspaces = workspaces;
                    focus_output(state, output_index);
                } else if state.focused_output.is_none() {
                    state.focused_output = Some(0);
                }
            }

            river::river_window_management::river_window_manager_v1::Event::SessionLocked => {
                state.session_locked = true;
                cancel_drag(state);
            }
            river::river_window_management::river_window_manager_v1::Event::SessionUnlocked => {
                state.session_locked = false;
                state.focus_dirty = true;
            }
            river::river_window_management::river_window_manager_v1::Event::ManageStart => {
                monitor_workspaces::reconcile(state);
                let current_output = output_at_pointer(state)
                    .or(state.focused_output)
                    .or_else(|| (!state.outputs.is_empty()).then_some(0));

                layer_shell::setup(state, qh);
                if let Some(output) = current_output {
                    if let Some(layer_output) = &state.outputs[output].layer_output {
                        layer_output.set_default();
                    }
                    assign_windows(state, output);
                }
                run_actions(state, manager, qh);
                if state.exit_requested {
                    manager.manage_finish();
                    return;
                }
                update_drag(state);
                layout(state);
                fullscreen::apply(state);
                vrr::apply(state);
                let mut schedule_nudge_restore = false;
                for window in &mut state.windows {
                    // The WM supplies focus borders, but no title bar.
                    window.river_window.use_ssd();
                    if window.node.is_none() {
                        let node = window.river_window.get_node(qh, ());
                        window.node = Some(node);
                    }

                    if window.fullscreen_output.is_some() {
                        continue;
                    }
                    if let Some((_, _, width, height)) = window.geometry {
                        window.river_window.set_tiled(if window.floating {
                            Edges::empty()
                        } else {
                            Edges::Top | Edges::Bottom | Edges::Left | Edges::Right
                        });
                        let proposal = (width, height);
                        if window.proposed_dimensions != Some(proposal) {
                            window.proposal_retried = false;
                        }
                        window.proposed_dimensions = Some(proposal);
                        if window.nudge {
                            window.nudge = false;
                            window.nudge_restore = true;
                            schedule_nudge_restore = true;
                            window
                                .river_window
                                .propose_dimensions(width, (height - 1).max(1));
                        } else {
                            window.river_window.propose_dimensions(width, height);
                        }
                    }
                }

                if schedule_nudge_restore {
                    state.dimension_retry_at =
                        Some(std::time::Instant::now() + std::time::Duration::from_millis(150));
                }

                if state.layer_focus_granted {
                    // Let the shell receive focus this sequence, including when an
                    // application is announced at the same time as its launcher.
                    state.focus_dirty = false;
                }
                if state.focus_dirty
                    && state.layer_focus != LayerFocus::Exclusive
                    && let Some(seat) = &state.seat
                {
                    if let Some(window) = state.windows.iter().find(|w| {
                        Some(w.river_window.id()) == state.focused_window && w.geometry.is_some()
                    }) {
                        seat.focus_window(&window.river_window);
                    } else {
                        seat.clear_focus();
                    }
                    state.focus_dirty = false;
                }

                install_bindings(state, qh);
                for binding in &state.bindings {
                    if state.session_locked {
                        binding.disable();
                    } else {
                        binding.enable();
                    }
                }
                for binding in &state.pointer_bindings {
                    if state.session_locked {
                        binding.disable();
                    } else {
                        binding.enable();
                    }
                }
                state.layer_focus_granted = false;
                manager.manage_finish();
            }

            river::river_window_management::river_window_manager_v1::Event::RenderStart => {
                if state.dimension_retry_at.is_none()
                    && state.windows.iter().any(needs_dimension_retry)
                {
                    state.dimension_retry_at =
                        Some(std::time::Instant::now() + std::time::Duration::from_millis(500));
                }
                for output in &mut state.outputs {
                    if !output.presentation_mode_set
                        && output.river_output.version() >= 4
                        && let Some(name) = output
                            .wl_global
                            .and_then(|global| state.wl_outputs.get(&global))
                            .and_then(|(_, name)| name.as_deref())
                    {
                        let mode = if state
                            .config
                            .async_outputs
                            .iter()
                            .any(|configured| configured == name)
                        {
                            river::river_window_management::river_output_v1::PresentationMode::Async
                        } else {
                            river::river_window_management::river_output_v1::PresentationMode::Vsync
                        };
                        output.river_output.set_presentation_mode(mode);
                        output.presentation_mode_set = true;
                    }
                }
                for window in state
                    .windows
                    .iter()
                    .filter(|w| !w.floating)
                    .chain(state.windows.iter().filter(|w| w.floating))
                {
                    if let Some(node) = &window.node {
                        if window.fullscreen_output.is_some() {
                            window.river_window.show();
                            node.place_top();
                            continue;
                        }
                        if let Some((x, y, _, _)) = window.geometry {
                            let output = &state.outputs[window.output.unwrap()];
                            let Some(area) = output.work_area() else {
                                window.river_window.hide();
                                continue;
                            };
                            let area = if window.floating {
                                area
                            } else {
                                state.config.appearance.viewport(area)
                            };
                            let (_, _, width, height) = window.geometry.unwrap();
                            let border = window.border_width;
                            let left = (x - border).max(area.x);
                            let right = (x + width + border).min(area.x + area.width);
                            let top = (y - border).max(area.y);
                            let bottom = (y + height + border).min(area.y + area.height);
                            if right <= left || bottom <= top {
                                window.river_window.hide();
                                continue;
                            }
                            let active = state.layer_focus == LayerFocus::None
                                && state.focused_window.as_ref() == Some(&window.river_window.id());
                            let color = if active {
                                state.config.appearance.active_border
                            } else {
                                state.config.appearance.inactive_border
                            };
                            let [r, g, b, a] = color.0;
                            window.river_window.set_borders(
                                Edges::Top | Edges::Bottom | Edges::Left | Edges::Right,
                                border,
                                r,
                                g,
                                b,
                                a,
                            );
                            window.river_window.show();
                            if window.river_window.version() >= 3 {
                                window
                                    .river_window
                                    .set_content_clip_box(0, 0, width, height);
                            }
                            if window.river_window.version() >= 2 {
                                window.river_window.set_clip_box(
                                    left - x,
                                    top - y,
                                    right - left,
                                    bottom - top,
                                );
                            }
                            node.set_position(x, y);
                            node.place_top();
                        } else {
                            window.river_window.hide();
                        }
                    }
                }

                manager.render_finish();
            }

            _ => {}
        }
    }

    wayland_client::event_created_child!(
        State,
        RiverWindowManagerV1,
        [
            6 => (RiverWindowV1, ()),
            7 => (RiverOutputV1, ()),
            8 => (RiverSeatV1, ()),
        ]
    );
}

impl Dispatch<RiverWindowV1, ()> for State {
    fn event(
        state: &mut Self,
        window: &RiverWindowV1,
        event: river::river_window_management::river_window_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        use river::river_window_management::river_window_v1::Event;
        match event {
            Event::Closed => remove_window(state, window.id()),
            Event::FullscreenRequested { .. } => {
                cancel_drag(state);
                if let Some(item) = state.windows.iter_mut().find(|w| w.river_window == *window) {
                    // Keep global workspace ownership authoritative over output hints.
                    item.fullscreen = true;
                }
            }
            Event::ExitFullscreenRequested => {
                if let Some(item) = state.windows.iter_mut().find(|w| w.river_window == *window) {
                    item.fullscreen = false;
                }
            }
            Event::Dimensions { width, height } => {
                if let Some(item) = state.windows.iter_mut().find(|w| w.river_window == *window) {
                    item.actual_dimensions = Some((width, height));
                }
            }
            Event::AppId { app_id } => {
                println!("Window {} app_id: {:?}", window.id(), app_id);
                if let Some(item) = state.windows.iter_mut().find(|w| w.river_window == *window) {
                    item.app_id = app_id;
                }
                let id = window.id();
                if let Some(gaming) = state.config.gaming_workspace.map(|number| number - 1)
                    && is_game_window(state, &id)
                    && let Some(source) = state
                        .windows
                        .iter()
                        .find(|item| item.river_window == *window)
                        .and_then(|item| item.output)
                {
                    monitor_workspaces::move_window_id(state, source, gaming, id);
                    let output = monitor_workspaces::owner(state, gaming).unwrap_or(source);
                    monitor_workspaces::select(state, output, gaming);
                }
            }
            Event::Title { title } => {
                println!("Window {} title: {:?}", window.id(), title);
            }
            Event::Parent { parent } => {
                if let Some(item) = state.windows.iter_mut().find(|w| w.river_window == *window) {
                    item.parent = parent.map(|p| p.id());
                    if item.parent.is_some()
                        && rules::resolve(
                            &state.config.rules,
                            item.app_id.as_deref(),
                            true,
                            state.config.float_dialogs,
                        )
                        .floating
                    {
                        item.floating = true;
                    }
                }
            }
            Event::PointerMoveRequested { seat } if state.seat.as_ref() == Some(&seat) => {
                state.pending_drag = Some((window.id(), PointerAction::Move));
            }
            Event::PointerResizeRequested {
                seat,
                edges: wayland_client::WEnum::Value(edges),
            } if state.seat.as_ref() == Some(&seat) => {
                state.pending_drag = Some((window.id(), PointerAction::ResizeEdges(edges)));
            }
            _ => {}
        }
    }

    wayland_client::event_created_child!(
        State,
        RiverWindowV1,
        [
            2 => (RiverNodeV1, ())
        ]
    );
}

impl Dispatch<RiverNodeV1, ()> for State {
    fn event(
        _state: &mut Self,
        _node: &RiverNodeV1,
        _event: river::river_window_management::river_node_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<RiverOutputV1, ()> for State {
    fn event(
        state: &mut Self,
        output: &RiverOutputV1,
        event: river::river_window_management::river_output_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        if matches!(
            event,
            river::river_window_management::river_output_v1::Event::Removed
        ) {
            if let Some(index) = state.outputs.iter().position(|o| o.river_output == *output) {
                cancel_drag(state);
                let removed = state.outputs.remove(index);
                if let Some(layer_output) = removed.layer_output {
                    layer_output.destroy();
                }
                let fallback = (!state.outputs.is_empty()).then_some(0);
                let focus_was_removed = state.windows.iter().any(|w| {
                    w.output == Some(index) && Some(w.river_window.id()) == state.focused_window
                });
                for window in &mut state.windows {
                    window.output = if window.output == Some(index) {
                        fallback
                    } else {
                        window
                            .output
                            .and_then(|i| scrolling::index_after_remove(i, index))
                    };
                }
                state.focused_output = state
                    .focused_output
                    .and_then(|i| scrolling::index_after_remove(i, index))
                    .or(fallback);
                if let Some(target) = fallback {
                    state.outputs[target].workspaces.absorb(removed.workspaces);
                    if focus_was_removed {
                        focus_output(state, target);
                    }
                } else {
                    state.detached_workspaces = Some(removed.workspaces);
                    state.focused_window = None;
                    state.focus_dirty = true;
                }
            }
            output.destroy();
            return;
        }
        if state.drag.is_some() {
            cancel_drag(state);
        }
        let Some(state_output) = state
            .outputs
            .iter_mut()
            .find(|item| item.river_output.id() == output.id())
        else {
            return;
        };

        match event {
            river::river_window_management::river_output_v1::Event::WlOutput { name } => {
                state_output.wl_global = Some(name);
            }
            river::river_window_management::river_output_v1::Event::Position { x, y } => {
                state_output.position = Some((x, y));

                println!("Output {} position: ({x}, {y})", output.id());
            }

            river::river_window_management::river_output_v1::Event::Dimensions {
                width,
                height,
            } => {
                state_output.dimensions = Some((width, height));

                println!("Output {} dimensions: {width}x{height}", output.id());
            }

            _ => {}
        }
    }
}

impl Dispatch<RiverSeatV1, ()> for State {
    fn event(
        state: &mut Self,
        seat: &RiverSeatV1,
        event: river::river_window_management::river_seat_v1::Event,
        _data: &(),
        _conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if matches!(
            event,
            river::river_window_management::river_seat_v1::Event::Removed
        ) {
            if state.seat.as_ref() == Some(seat) {
                for binding in state.bindings.drain(..) {
                    binding.destroy();
                }
                for binding in state.pointer_bindings.drain(..) {
                    binding.destroy();
                }
                cancel_drag(state);
                if let Some(layer_seat) = state.layer_seat.take() {
                    layer_seat.destroy();
                }
                state.layer_focus = LayerFocus::None;
                state.layer_focus_granted = false;
                state.seat = None;
                state.pointer_position = None;
                state.pointer_window = None;
            }
            seat.destroy();
            return;
        }
        if state.seat.as_ref().is_some_and(|active| active != seat) {
            return;
        }
        state.seat = Some(seat.clone());
        layer_shell::setup(state, qh);

        match event {
            river::river_window_management::river_seat_v1::Event::PointerEnter { window } => {
                state.pointer_window = Some(window.id());
            }
            river::river_window_management::river_seat_v1::Event::PointerLeave => {
                state.pointer_window = None;
            }
            river::river_window_management::river_seat_v1::Event::OpDelta { dx, dy } => {
                if state.drag.is_some() {
                    state.drag_delta = Some((dx, dy));
                }
            }
            river::river_window_management::river_seat_v1::Event::OpRelease => {
                state.end_drag = true;
            }
            river::river_window_management::river_seat_v1::Event::PointerPosition { x, y } => {
                state.pointer_position = Some((x, y));

                update_focused_output(state, x, y);
            }

            river::river_window_management::river_seat_v1::Event::WindowInteraction { window } => {
                let Some(window_index) = state
                    .windows
                    .iter()
                    .position(|item| item.river_window.id() == window.id())
                else {
                    println!("Window interaction for unknown window");
                    return;
                };

                if state.scratchpad_visible && state.scratchpad.windows.contains(&window.id()) {
                    state.scratchpad.focused = Some(window.id());
                    focus_scratchpad(state);
                    return;
                }

                if let Some(output) = state.windows[window_index].output
                    && state.outputs[output].workspaces.focus(&window.id())
                {
                    focus_output(state, output);
                }
            }

            _ => {}
        }
    }
}

impl Dispatch<RiverPointerBindingV1, PointerAction> for State {
    fn event(
        state: &mut State,
        _binding: &RiverPointerBindingV1,
        event: river::river_window_management::river_pointer_binding_v1::Event,
        action: &PointerAction,
        _conn: &Connection,
        _qh: &QueueHandle<State>,
    ) {
        if matches!(
            event,
            river::river_window_management::river_pointer_binding_v1::Event::Pressed
        ) && let Some(window) = &state.pointer_window
        {
            state.pending_drag = Some((window.clone(), *action));
        }
    }
}

impl Dispatch<RiverXkbBindingsV1, ()> for State {
    fn event(
        state: &mut State,
        _bindings: &RiverXkbBindingsV1,
        _event: river::river_xkb_bindings::river_xkb_bindings_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<State>,
    ) {
        state.xkb_bindings = Some(_bindings.clone());
    }

    // event_created_child! ...
}

impl Dispatch<RiverXkbBindingV1, Action> for State {
    fn event(
        state: &mut State,
        _binding: &RiverXkbBindingV1,
        event: river::river_xkb_bindings::river_xkb_binding_v1::Event,
        action: &Action,
        _conn: &Connection,
        _qh: &QueueHandle<State>,
    ) {
        if matches!(
            event,
            river::river_xkb_bindings::river_xkb_binding_v1::Event::Pressed
        ) {
            state.actions.push(*action);
        }
    }
}

impl Dispatch<RiverXkbBindingsSeatV1, ()> for State {
    fn event(
        _state: &mut State,
        _seat: &RiverXkbBindingsSeatV1,
        _event: river::river_xkb_bindings::river_xkb_bindings_seat_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<State>,
    ) {
    }
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
