//! Global registry: binds River, layer-shell and xkb-bindings globals.
use crate::*;

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
