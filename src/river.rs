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
