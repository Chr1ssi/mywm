//! Workspace ownership: one fixed workspace per monitor, dynamically created
//! extras that live on the monitor they were created on, and a gaming
//! workspace that exists only while a game runs.
use crate::workspaces::{GAMING, Kind, MAX_NUMBER};
use crate::{State, cancel_drag, focus_output};
use std::collections::HashSet;
use wayland_client::{
    Connection, Dispatch, Proxy, QueueHandle,
    protocol::wl_output::{self, WlOutput},
};

pub fn output_name(state: &State, output: usize) -> Option<&str> {
    state
        .outputs
        .get(output)?
        .wl_global
        .and_then(|id| state.wl_outputs.get(&id))
        .and_then(|(_, name)| name.as_deref())
}

/// Number of the fixed workspace of `output`: its position in
/// `workspace_outputs`, or the next free number for monitors not listed there.
pub fn home_number(state: &State, output: usize) -> usize {
    let listed = &state.config.workspace_outputs;
    let position = |index| {
        output_name(state, index).and_then(|name| listed.iter().position(|item| item == name))
    };
    if let Some(position) = position(output) {
        return position + 1;
    }
    listed.len() + (0..output).filter(|index| position(*index).is_none()).count() + 1
}

/// The monitor showing the gaming workspace: `gaming_output`, else the monitor
/// with workspace 1.
pub fn gaming_output(state: &State) -> Option<usize> {
    state
        .config
        .gaming_output
        .as_deref()
        .and_then(|wanted| {
            (0..state.outputs.len()).find(|index| output_name(state, *index) == Some(wanted))
        })
        .or_else(|| (0..state.outputs.len()).find(|index| home_number(state, *index) == 1))
        .or_else(|| (!state.outputs.is_empty()).then_some(0))
}

/// The monitor a workspace number lives on, if that workspace exists. The
/// gaming workspace always belongs to the gaming monitor.
pub fn owner(state: &State, number: usize) -> Option<usize> {
    if number == GAMING {
        return gaming_output(state);
    }
    state
        .outputs
        .iter()
        .position(|output| output.workspaces.contains(number))
}

fn used_numbers(state: &State) -> HashSet<usize> {
    let mut used: HashSet<_> = (1..=state.config.workspace_outputs.len()).collect();
    for (index, output) in state.outputs.iter().enumerate() {
        used.insert(home_number(state, index));
        used.extend(output.workspaces.numbers());
    }
    used
}

fn free_number(state: &State) -> Option<usize> {
    let used = used_numbers(state);
    (1..=MAX_NUMBER).find(|number| !used.contains(number))
}

/// Create an extra workspace on `output` (or reuse its empty one) and return
/// its number. `None` if all numbers are taken.
pub fn create(state: &mut State, output: usize) -> Option<usize> {
    if let Some(empty) = state.outputs[output]
        .workspaces
        .entries
        .iter()
        .find(|entry| entry.kind == Kind::Extra && entry.windows.is_empty())
    {
        return Some(empty.number);
    }
    let number = free_number(state)?;
    state.outputs[output]
        .workspaces
        .ensure(number, Kind::Extra);
    Some(number)
}

/// Make sure the gaming workspace exists on the gaming monitor; returns it.
pub fn ensure_gaming(state: &mut State) -> Option<usize> {
    let output = gaming_output(state)?;
    state.outputs[output]
        .workspaces
        .ensure(GAMING, Kind::Gaming);
    Some(output)
}

/// Give every monitor its home workspace under the right number, and bring
/// workspaces back to their owning monitor (after hotplug, or once a
/// connector name became known). Missing monitors' workspaces stay on the
/// remaining ones until they return.
pub fn reconcile(state: &mut State) {
    if state.outputs.is_empty() {
        return;
    }
    let mut changed = false;
    let homes: Vec<_> = (0..state.outputs.len())
        .map(|index| home_number(state, index))
        .collect();
    // Extras never keep a number that a monitor's home workspace needs.
    for output in 0..state.outputs.len() {
        let colliding: Vec<_> = state.outputs[output]
            .workspaces
            .entries
            .iter()
            .filter(|entry| entry.kind == Kind::Extra && homes.contains(&entry.number))
            .map(|entry| entry.number)
            .collect();
        for old in colliding {
            if let Some(new) = free_number(state) {
                state.outputs[output].workspaces.renumber(old, new);
                changed = true;
            }
        }
    }
    for (output, home) in homes.iter().enumerate() {
        let workspaces = &mut state.outputs[output].workspaces;
        if workspaces.home != *home {
            workspaces.renumber(workspaces.home, *home);
            changed = true;
        }
        workspaces.ensure(*home, Kind::Home);
    }
    let gaming = gaming_output(state).unwrap();
    for source in 0..state.outputs.len() {
        let misplaced: Vec<_> = state.outputs[source]
            .workspaces
            .entries
            .iter()
            .filter_map(|entry| {
                let target = match entry.kind {
                    Kind::Home => homes.iter().position(|home| *home == entry.number)?,
                    Kind::Gaming => gaming,
                    Kind::Extra => return None,
                };
                (target != source).then_some((entry.number, target))
            })
            .collect();
        for (number, target) in misplaced {
            let Some(moved) = state.outputs[source].workspaces.take(number) else {
                continue;
            };
            for window in &mut state.windows {
                if moved.windows.contains(&window.river_window.id()) {
                    window.output = Some(target);
                }
            }
            state.outputs[target].workspaces.insert(moved);
            changed = true;
        }
    }
    if changed {
        cancel_drag(state);
        let output = state
            .focused_output
            .filter(|index| *index < state.outputs.len())
            .unwrap_or(gaming);
        focus_output(state, output);
    }
}

/// Remove extras that were left empty and the gaming workspace once its last
/// window is gone; the monitor returns to the workspace it came from.
pub fn prune(state: &mut State) {
    let homes: Vec<_> = (0..state.outputs.len())
        .map(|index| home_number(state, index))
        .collect();
    for output in 0..state.outputs.len() {
        // A missing monitor's empty home workspace is no longer needed.
        for entry in &mut state.outputs[output].workspaces.entries {
            if entry.kind == Kind::Home && entry.windows.is_empty() && !homes.contains(&entry.number)
            {
                entry.kind = Kind::Extra;
            }
        }
        if state.outputs[output].workspaces.prune()
            && (state.focused_output == Some(output)
                || crate::output_at_pointer(state) == Some(output))
        {
            cancel_drag(state);
            focus_output(state, output);
        }
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
    let Some(target) = owner(state, workspace) else {
        return;
    };
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
    if crate::output_at_pointer(state) != Some(output)
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
