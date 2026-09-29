//! Per-workspace layout computation (scrolling columns, floating windows, borders).
use crate::*;

pub(crate) fn layout(state: &mut State) {
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
