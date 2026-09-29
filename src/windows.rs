//! Window assignment, removal, game detection and the size-proposal retry logic.
use crate::*;

/// How long a window may disagree with the proposed size before we intervene.
pub(crate) const DIMENSION_RETRY_DELAY: std::time::Duration = std::time::Duration::from_millis(120);
/// Size difference (in logical pixels) above which a mismatch counts as stale geometry.
const STALE_GEOMETRY_THRESHOLD: i32 = 48;
/// Delay between the one-pixel nudge and proposing the real size again.
pub(crate) const NUDGE_RESTORE_DELAY: std::time::Duration = std::time::Duration::from_millis(80);

pub(crate) fn is_game_window(state: &State, id: &ObjectId) -> bool {
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

pub(crate) fn needs_dimension_retry(window: &Window) -> bool {
    window.fullscreen_output.is_none()
        && window.actual_dimensions.is_some()
        && window.actual_dimensions != window.proposed_dimensions
        && !window.proposal_retried
}

/// Whether the reported size is so far from the proposal that the window
/// cannot merely be snapping to a size grid.
fn is_stale_geometry(window: &Window) -> bool {
    match (window.actual_dimensions, window.proposed_dimensions) {
        (Some((aw, ah)), Some((pw, ph))) => {
            (aw - pw).abs() > STALE_GEOMETRY_THRESHOLD || (ah - ph).abs() > STALE_GEOMETRY_THRESHOLD
        }
        _ => false,
    }
}

pub(crate) fn run_dimension_retry(state: &mut State) {
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
            // Terminals round to whole cells, so small differences are normal
            // and must not make the window jump.
            window.nudge = is_stale_geometry(window);
            retry_layout = true;
        }
    }
    if retry_layout && let Some(manager) = &state.manager {
        manager.manage_dirty();
    }
}

/// The gaming workspace is reserved for games and their dialogs.
pub(crate) fn workspace_accepts(state: &State, workspace: usize, id: &ObjectId) -> bool {
    workspace != GAMING || is_game_window(state, id)
}

pub(crate) fn assign_windows(state: &mut State, default_output: usize) {
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
            let (output, active) = inherited.unwrap_or((
                default_output,
                state.outputs[default_output].workspaces.active,
            ));
            let id = window.river_window.id();
            let mut workspace = placement
                .workspace
                .filter(|number| monitor_workspaces::owner(state, *number).is_some())
                .unwrap_or(active);
            let game = is_game_window(state, &id);
            let mut output = output;
            if game && let Some(gaming) = monitor_workspaces::ensure_gaming(state) {
                output = gaming;
                workspace = GAMING;
            } else if let Some(owner) = monitor_workspaces::owner(state, workspace) {
                output = owner;
            }
            if workspace == GAMING && !game {
                // Only games start on the gaming workspace; show the monitor's own instead.
                workspace = state.outputs[output].workspaces.home;
                state.outputs[output].workspaces.select(workspace);
            } else if game {
                state.outputs[output].workspaces.select(GAMING);
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

pub(crate) fn remove_window(state: &mut State, window_id: wayland_client::backend::ObjectId) {
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
