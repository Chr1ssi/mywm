mod river;

use river::river_window_management::{
    river_node_v1::RiverNodeV1,
    river_output_v1::RiverOutputV1,
    river_seat_v1::RiverSeatV1,
    river_window_manager_v1::RiverWindowManagerV1,
    river_window_v1::RiverWindowV1,
};

use wayland_client::{
    Connection,
    Dispatch,
    Proxy,
    QueueHandle,
    protocol::wl_registry,
};

struct Window {
    river_window: RiverWindowV1,
    node: Option<RiverNodeV1>,
    geometry: Option<(i32, i32, i32, i32)>,
    output: Option<...>,
}

struct Output {
    river_output: RiverOutputV1,
    position: Option<(i32, i32)>,
    dimensions: Option<(i32, i32)>,
}

struct State {
    windows: Vec<Window>,
    outputs: Vec<Output>,
}

fn layout(state: &mut State) {
    let Some(output) = state.outputs.first() else {
        return;
    };

    let Some((output_width, output_height)) = output.dimensions else {
        return;
    };

    let window_count = state.windows.len();

    if window_count == 0 {
        return;
    }

    let window_height = output_height / window_count as i32;

    for (index, window) in state.windows.iter_mut().enumerate() {
        let x = output.position.map_or(0, |(x, _)| x);
        let y = output.position.map_or(0, |(_, y)| y)
            + index as i32 * window_height;

        window.geometry = Some((x, y, output_width, window_height));

        println!(
            "Layout window {}: x={} y={} width={} height={}",
            index,
            x,
            y,
            output_width,
            window_height
        );
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for State {
    fn event(
        _state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _data: &(),
        _conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            println!("[{name}] {interface} (v{version})");

            if interface == "river_window_manager_v1" {
                println!("Binding river_window_manager_v1 v{version}");

                registry.bind::<RiverWindowManagerV1, _, _>(
                    name,
                    version.min(5),
                    qh,
                    (),
                );
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
        println!("river_window_manager_v1 event: {event:?}");

        match event {
            river::river_window_management::river_window_manager_v1::Event::Window { id } => {
                println!("New window");

                state.windows.push(Window {
                    river_window: id,
                    node: None,
                    geometry: None,
                });
            }

            river::river_window_management::river_window_manager_v1::Event::Output { id } => {
                println!("New output");

                state.outputs.push(Output {
                    river_output: id,
                    position: None,
                    dimensions: None,
                });
            }

            river::river_window_management::river_window_manager_v1::Event::ManageStart => {
                println!("Manage sequence started");

                for window in &mut state.windows {
                    if window.node.is_none() {
                        let node = window.river_window.get_node(qh, ());
                        println!("Got node: {node:?}");
                        window.node = Some(node);
                    }

                    if let Some((_, _, width, height)) = window.geometry {
                        window.river_window.propose_dimensions(width, height);
                    }
                }

                manager.manage_finish();
            }

            river::river_window_management::river_window_manager_v1::Event::RenderStart => {
                println!("Render sequence started");

                layout(state);

                for window in &state.windows {
                    if let Some(node) = &window.node {
                        if let Some((x, y, _, _)) = window.geometry {
                            node.set_position(x, y);
                            node.place_top();
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
        _state: &mut Self,
        _window: &RiverWindowV1,
        event: river::river_window_management::river_window_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        println!("river_window_v1 event: {event:?}");
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
        let Some(state_output) = state
            .outputs
            .iter_mut()
            .find(|item| item.river_output.id() == output.id())
        else {
            return;
        };

        match event {
            river::river_window_management::river_output_v1::Event::Position { x, y } => {
                state_output.position = Some((x, y));

                println!("Output {} position: ({x}, {y})", output.id());
            }

            river::river_window_management::river_output_v1::Event::Dimensions {
                width,
                height,
            } => {
                state_output.dimensions = Some((width, height));

                println!(
                    "Output {} dimensions: {width}x{height}",
                    output.id()
                );
            }

            _ => {}
        }
    }
}

impl Dispatch<RiverSeatV1, ()> for State {
    fn event(
        _state: &mut Self,
        _seat: &RiverSeatV1,
        event: river::river_window_management::river_seat_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        println!("river_seat_v1 event: {event:?}");
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("mywm starting");

    let conn = Connection::connect_to_env()?;
    println!("connected to Wayland");

    let mut event_queue = conn.new_event_queue();
    let qh = event_queue.handle();

    let display = conn.display();
    display.get_registry(&qh, ());

    println!("Advertised globals:");

    let mut state = State {
        windows: Vec::new(),
        outputs: Vec::new(),
    };

    event_queue.roundtrip(&mut state)?;

    println!("Entering event loop");

    loop {
        event_queue.blocking_dispatch(&mut state)?;
    }
}
