//! Pointer-driven window move and resize handling.
use crate::*;

pub(crate) fn cancel_drag(state: &mut State) {
    state.pending_drag = None;
    state.end_drag = true;
}

pub(crate) fn update_drag(state: &mut State) {
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
