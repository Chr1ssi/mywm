//! Render-node events (unused).
use crate::*;

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
