use crate::{State, cancel_drag, focus_output};
use wayland_client::{
    Connection, Dispatch, Proxy, QueueHandle,
    protocol::wl_output::{self, WlOutput},
};

pub fn owner(state: &State, workspace: usize) -> Option<usize> {
    if state.config.workspace_outputs.is_empty() {
        return None;
    }
    let preferred = state
        .config
        .workspace_outputs
        .iter()
        .find(|(_, numbers)| numbers.contains(&(workspace + 1)))
        .map(|(name, _)| name);
    preferred
        .and_then(|preferred| {
            state.outputs.iter().position(|output| {
                output
                    .wl_global
                    .and_then(|id| state.wl_outputs.get(&id))
                    .and_then(|(_, name)| name.as_ref())
                    == Some(preferred)
            })
        })
        .or_else(|| (!state.outputs.is_empty()).then_some(0))
}
pub fn mask(state: &State, output: usize) -> u32 {
    (0..state.config.workspaces).fold(0, |mask, workspace| {
        mask | if owner(state, workspace).is_none_or(|i| i == output) {
            1 << workspace
        } else {
            0
        }
    })
}

/// Keep the existing per-output storage, but each global workspace has one owner.
/// Missing connectors temporarily fall back to the first remaining output.
pub fn reconcile(state: &mut State) {
    if state.config.workspace_outputs.is_empty() || state.outputs.is_empty() {
        return;
    }
    let mut changed = false;
    for workspace in 0..state.config.workspaces {
        let target = owner(state, workspace).unwrap();
        for source in 0..state.outputs.len() {
            if source == target
                || state.outputs[source].workspaces.entries[workspace]
                    .windows
                    .is_empty()
            {
                continue;
            }
            let moved = std::mem::take(&mut state.outputs[source].workspaces.entries[workspace]);
            for window in &mut state.windows {
                if moved.windows.contains(&window.river_window.id()) {
                    window.output = Some(target);
                }
            }
            let destination = &mut state.outputs[target].workspaces.entries[workspace];
            if destination.windows.is_empty() {
                *destination = moved;
            } else {
                destination.windows.extend(moved.windows);
                if destination.focused.is_none() {
                    destination.focused = moved.focused;
                }
            }
            changed = true;
        }
    }
    for output in 0..state.outputs.len() {
        let allowed = mask(state, output);
        if allowed != 0 && allowed & (1 << state.outputs[output].workspaces.active) == 0 {
            state.outputs[output]
                .workspaces
                .select(allowed.trailing_zeros() as usize);
            changed = true;
        }
    }
    if changed {
        cancel_drag(state);
        let output = state
            .focused_output
            .filter(|i| mask(state, *i) != 0)
            .unwrap_or_else(|| owner(state, 0).unwrap());
        focus_output(state, output);
    }
}

pub fn move_window(state: &mut State, source: usize, workspace: usize) {
    let Some(id) = state.focused_window.clone() else {
        return;
    };
    move_window_id(state, source, workspace, id);
}

pub fn move_window_id(
    state: &mut State,
    source: usize,
    workspace: usize,
    id: wayland_client::backend::ObjectId,
) {
    let target = owner(state, workspace).unwrap_or(source);
    if source == target {
        let current = state.outputs[source].workspaces.location(&id);
        if current != Some(workspace) {
            state.outputs[source].workspaces.remove(&id);
            state.outputs[source]
                .workspaces
                .add_to(workspace, id.clone());
        }
    } else {
        state.outputs[source].workspaces.remove(&id);
        state.outputs[target]
            .workspaces
            .add_to(workspace, id.clone());
        if let Some(window) = state.windows.iter_mut().find(|w| w.river_window.id() == id) {
            window.output = Some(target);
            // Floating coordinates belong to the source work area. Recenter on target.
            window.floating_rect = None;
        }
    }
    focus_output(state, source);
}

impl Dispatch<WlOutput, u32> for State {
    fn event(
        state: &mut Self,
        _: &WlOutput,
        event: wl_output::Event,
        global: &u32,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_output::Event::Name { name } = event {
            if let Some((_, stored)) = state.wl_outputs.get_mut(global) {
                *stored = Some(name);
            }
            if let Some(manager) = &state.manager {
                manager.manage_dirty();
            }
        }
    }
}

pub fn select(state: &mut State, output: usize, workspace: usize) {
    state.outputs[output].workspaces.select(workspace);
    if !state.config.workspace_outputs.is_empty()
        && crate::output_at_pointer(state) != Some(output)
        && let Some(area) = state.outputs[output].work_area()
        && let Some(seat) = &state.seat
        && seat.version() >= 3
    {
        let position = (area.x + area.width / 2, area.y + area.height / 2);
        seat.pointer_warp(position.0, position.1);
        state.pointer_position = Some(position);
    }
    focus_output(state, output);
}
