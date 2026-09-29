//! Per-window events (close, fullscreen, app id, dimensions, ...).
use crate::*;

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
                if is_game_window(state, &id)
                    && let Some(source) = state
                        .windows
                        .iter()
                        .find(|item| item.river_window == *window)
                        .and_then(|item| item.output)
                    && let Some(output) = monitor_workspaces::ensure_gaming(state)
                {
                    monitor_workspaces::move_window_id(state, source, GAMING, id);
                    monitor_workspaces::select(state, output, GAMING);
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
