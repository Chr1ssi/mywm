//! Output focus, adjacency and pointer-to-output helpers.
use crate::*;

/// The workspace `direction` steps away on this monitor, wrapping around.
pub(crate) fn relative_workspace(state: &State, output: usize, direction: isize) -> Option<usize> {
    let workspaces = &state.outputs[output].workspaces;
    let numbers: Vec<_> = workspaces.numbers().collect();
    let position = numbers
        .iter()
        .position(|number| *number == workspaces.active)?;
    Some(numbers[(position as isize + direction).rem_euclid(numbers.len() as isize) as usize])
}

pub(crate) fn focus_output(state: &mut State, output: usize) {
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

pub(crate) fn focus_scratchpad(state: &mut State) {
    state.focused_window = state.scratchpad.focused.clone();
    state.focus_dirty = true;
}

pub(crate) fn adjacent_output(
    state: &State,
    source: usize,
    direction: OutputDirection,
) -> Option<usize> {
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

pub(crate) fn warp_to_output(state: &mut State, output: usize) {
    if let Some(area) = state.outputs[output].work_area()
        && let Some(seat) = &state.seat
        && seat.version() >= 3
    {
        let position = (area.x + area.width / 2, area.y + area.height / 2);
        seat.pointer_warp(position.0, position.1);
        state.pointer_position = Some(position);
    }
}

pub(crate) fn update_focused_output(state: &mut State, pointer_x: i32, pointer_y: i32) {
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

pub(crate) fn output_at_pointer(state: &State) -> Option<usize> {
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
