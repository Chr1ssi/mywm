#!/usr/bin/env python3
"""Verify flat/neutral pointer setup, hotplug, unsupported devices and removal."""
import struct
import xml.etree.ElementTree as ET
from river_protocol import RiverPeer, has


class PointerPeer(RiverPeer):
    def __init__(self):
        super().__init__()
        for name in ("input-management", "libinput-config"):
            path = f"/usr/share/river-protocols/stable/river-{name}-v1.xml"
            for interface in ET.parse(path).getroot().findall("interface"):
                self.interfaces[interface.attrib["name"]] = interface

    def request(self):
        result = super().request()
        obj, name, args = result
        if self.objects[obj] == "wl_display" and name == "get_registry":
            self.event(args["registry"], "global", 10, "river_input_manager_v1", 2)
            self.event(args["registry"], "global", 11, "river_libinput_config_v1", 2)
        if name in ("set_accel_profile", "set_accel_speed"):
            self.event(args["result"], "success")
        return result


peer = PointerPeer()
try:
    while "river_libinput_config_v1" not in peer.objects.values():
        peer.request()
    manager = next(k for k, v in peer.objects.items() if v == "river_libinput_config_v1")
    inputs = next(k for k, v in peer.objects.items() if v == "river_input_manager_v1")

    def device(profiles):
        peer.server_id += 1
        input_id = peer.server_id
        peer.event(inputs, "input_device", input_id)
        peer.server_id += 1
        dev = peer.server_id
        peer.event(manager, "libinput_device", dev)
        peer.event(dev, "input_device", input_id)
        peer.event(dev, "accel_profiles_support", profiles)
        return dev

    for _ in range(2):  # Initial device and a later hotplug.
        dev = device(3)
        requests = peer.cycle()
        assert has(requests, "set_accel_profile", dev, profile=1)
        assert has(requests, "set_accel_speed", dev, speed=struct.pack("=d", 0.0))
        assert not has(peer.cycle(), "set_accel_profile", dev), "Repeated configuration"
        peer.event(dev, "removed")
        assert has(peer.cycle(), "destroy", dev)
    for profiles in (0, 2):
        dev = device(profiles)
        requests = peer.cycle()
        assert not has(requests, "set_accel_profile", dev)
        assert not has(requests, "set_accel_speed", dev)
    print("Pointer protocol passed: flat/neutral, hotplug, unsupported devices, removal")
finally:
    peer.close()
