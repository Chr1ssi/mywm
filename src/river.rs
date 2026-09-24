pub mod river_window_management {
    use wayland_client;
    use wayland_client::protocol::*;

    pub mod __interfaces {
        use wayland_client::protocol::__interfaces::*;

        wayland_scanner::generate_interfaces!(
            "/usr/share/river-protocols/stable/river-window-management-v1.xml"
        );
    }

    use self::__interfaces::*;

    wayland_scanner::generate_client_code!(
        "/usr/share/river-protocols/stable/river-window-management-v1.xml"
    );
}

pub mod river_xkb_bindings {
    use wayland_client;

    pub mod __interfaces {

        use super::super::river_window_management::__interfaces::{
            RIVER_SEAT_V1_INTERFACE, river_seat_v1_interface,
        };

        wayland_scanner::generate_interfaces!(
            "/usr/share/river-protocols/stable/river-xkb-bindings-v1.xml"
        );
    }

    use self::__interfaces::*;
    use super::river_window_management::river_seat_v1;

    wayland_scanner::generate_client_code!(
        "/usr/share/river-protocols/stable/river-xkb-bindings-v1.xml"
    );
}

pub mod river_layer_shell {
    use wayland_client;
    pub mod __interfaces {
        use super::super::river_window_management::__interfaces::{
            RIVER_OUTPUT_V1_INTERFACE, RIVER_SEAT_V1_INTERFACE, river_output_v1_interface,
            river_seat_v1_interface,
        };
        wayland_scanner::generate_interfaces!(
            "/usr/share/river-protocols/stable/river-layer-shell-v1.xml"
        );
    }

    use self::__interfaces::*;
    use super::river_window_management::{river_output_v1, river_seat_v1};
    wayland_scanner::generate_client_code!(
        "/usr/share/river-protocols/stable/river-layer-shell-v1.xml"
    );
}

pub mod river_input_management {
    use wayland_client;
    use wayland_client::protocol::wl_output;
    pub mod __interfaces {
        use wayland_client::protocol::__interfaces::*;
        wayland_scanner::generate_interfaces!(
            "/usr/share/river-protocols/stable/river-input-management-v1.xml"
        );
    }
    use self::__interfaces::*;
    wayland_scanner::generate_client_code!(
        "/usr/share/river-protocols/stable/river-input-management-v1.xml"
    );
}
pub mod river_xkb_config {
    use wayland_client;
    pub mod __interfaces {
        use super::super::river_input_management::__interfaces::*;
        wayland_scanner::generate_interfaces!(
            "/usr/share/river-protocols/stable/river-xkb-config-v1.xml"
        );
    }
    use self::__interfaces::*;
    use super::river_input_management::river_input_device_v1;
    wayland_scanner::generate_client_code!(
        "/usr/share/river-protocols/stable/river-xkb-config-v1.xml"
    );
}

pub mod river_libinput_config {
    use wayland_client;
    pub mod __interfaces {
        use super::super::river_input_management::__interfaces::*;
        wayland_scanner::generate_interfaces!(
            "/usr/share/river-protocols/stable/river-libinput-config-v1.xml"
        );
    }
    use self::__interfaces::*;
    use super::river_input_management::river_input_device_v1;
    wayland_scanner::generate_client_code!(
        "/usr/share/river-protocols/stable/river-libinput-config-v1.xml"
    );
}
