use crate::{
    State,
    river::river_libinput_config::{
        river_libinput_config_v1::{self, RiverLibinputConfigV1},
        river_libinput_device_v1::{self, RiverLibinputDeviceV1},
        river_libinput_result_v1::{self, RiverLibinputResultV1},
    },
};
use wayland_client::{Connection, Dispatch, QueueHandle};

impl Dispatch<RiverLibinputConfigV1, ()> for State {
    fn event(
        _: &mut Self,
        object: &RiverLibinputConfigV1,
        event: river_libinput_config_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let river_libinput_config_v1::Event::Finished = event {
            object.destroy();
        }
    }
    wayland_client::event_created_child!(State, RiverLibinputConfigV1, [1 => (RiverLibinputDeviceV1, ())]);
}

impl Dispatch<RiverLibinputDeviceV1, ()> for State {
    fn event(
        _: &mut Self,
        object: &RiverLibinputDeviceV1,
        event: river_libinput_device_v1::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        match event {
            // Support is announced once per device, including hotplugged devices.
            river_libinput_device_v1::Event::AccelProfilesSupport { profiles }
                if u32::from(profiles) & river_libinput_device_v1::AccelProfiles::Flat.bits()
                    != 0 =>
            {
                object.set_accel_profile(
                    river_libinput_device_v1::AccelProfile::Flat,
                    qh,
                    "profile",
                );
                object.set_accel_speed(0.0_f64.to_ne_bytes().to_vec(), qh, "speed");
            }
            river_libinput_device_v1::Event::Removed => object.destroy(),
            _ => {}
        }
    }
}

impl Dispatch<RiverLibinputResultV1, &'static str> for State {
    fn event(
        _: &mut Self,
        _: &RiverLibinputResultV1,
        event: river_libinput_result_v1::Event,
        setting: &&'static str,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        // Result events are destructors; no explicit destroy request is needed.
        if !matches!(event, river_libinput_result_v1::Event::Success) {
            eprintln!("River could not apply pointer {setting}: {event:?}");
        }
    }
}
