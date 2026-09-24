#!/usr/bin/env python3
"""Check mywm's real Wayland requests against a minimal River protocol peer.

Run after cargo build. Uses only Python's standard library and installed XMLs;
this tests protocol integration, not rendering or physical keyboard matching.
"""
import os
from pathlib import Path
import socket
import struct
import subprocess
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[1]


class RiverPeer:
    def __init__(self, config=None, layer_shell=False, output_names=None):
        self.output_names = output_names or {}
        self.interfaces = {}
        for path in ["/usr/share/wayland/wayland.xml",
                     "/usr/share/river-protocols/stable/river-window-management-v1.xml",
                     "/usr/share/river-protocols/stable/river-xkb-bindings-v1.xml",
                     "/usr/share/river-protocols/stable/river-layer-shell-v1.xml"]:
            for interface in ET.parse(path).getroot().findall("interface"):
                self.interfaces[interface.attrib["name"]] = interface
        self.objects = {1: "wl_display"}
        self.bindings = {}
        self.pointer_bindings = {}
        self.nodes = {}
        self.layer_shell = layer_shell
        self.layer_outputs = {}
        self.layer_seats = {}
        self.server_id = 0xfeffffff
        self.socket, client = socket.socketpair()
        self.socket.settimeout(5)
        env = dict(os.environ, WAYLAND_SOCKET=str(client.fileno()),
                   MYWM_CONFIG=str(config or ROOT / "tests/fixtures/plain.toml"))
        env.pop("MYWM_SOCKET", None)
        self.process = subprocess.Popen([str(ROOT / "target/debug/mywm")], env=env,
                                        pass_fds=(client.fileno(),), stdout=subprocess.PIPE,
                                        stderr=subprocess.STDOUT, text=True)
        client.close()
        self.manager = None

    def close(self):
        if self.process.poll() is None:
            self.process.terminate()
        output = self.process.communicate(timeout=5)[0]
        self.socket.close()
        if output:
            print(output.strip())

    def receive_bytes(self, size):
        data = b""
        while len(data) < size:
            part = self.socket.recv(size - len(data))
            if not part:
                raise AssertionError("mywm disconnected unexpectedly")
            data += part
        return data

    def event(self, object_id, name, *values):
        events = self.interfaces[self.objects[object_id]].findall("event")
        opcode, event = next((i, e) for i, e in enumerate(events) if e.attrib["name"] == name)
        payload = b""
        for arg, value in zip(event.findall("arg"), values, strict=True):
            kind = arg.attrib["type"]
            if kind == "string":
                encoded = value.encode() + b"\0"
                payload += struct.pack("=I", len(encoded)) + encoded + bytes(-len(encoded) % 4)
            else:
                payload += struct.pack("=i" if kind == "int" else "=I", value)
            if kind == "new_id":
                self.objects[value] = arg.attrib["interface"]
        self.socket.sendall(struct.pack("=II", object_id, ((8 + len(payload)) << 16) | opcode) + payload)

    def request(self):
        object_id, header = struct.unpack("=II", self.receive_bytes(8))
        payload = self.receive_bytes((header >> 16) - 8)
        interface = self.objects[object_id]
        request = self.interfaces[interface].findall("request")[header & 0xffff]
        name = request.attrib["name"]
        offset = 0

        def read(kind):
            nonlocal offset
            value = struct.unpack_from("=i" if kind == "int" else "=I", payload, offset)[0]
            offset += 4
            if kind in ("string", "array"):
                length = value
                value = (payload[offset:offset + length - 1].decode() if kind == "string"
                         else payload[offset:offset + length])
                offset += (length + 3) & ~3
            return value

        arguments = {}
        for arg in request.findall("arg"):
            kind = arg.attrib["type"]
            child_interface = arg.get("interface")
            if kind == "new_id" and child_interface is None:
                child_interface = read("string")
                read("uint")  # dynamically bound interface version
            value = read(kind)
            arguments[arg.attrib["name"]] = value
            if kind == "new_id":
                self.objects[value] = child_interface
                if child_interface == "river_window_manager_v1":
                    self.manager = value
        assert offset == len(payload), (name, offset, payload)
        if interface == "wl_display" and name == "get_registry":
            registry = arguments["registry"]
            self.event(registry, "global", 1, "river_window_manager_v1", 5)
            self.event(registry, "global", 2, "river_xkb_bindings_v1", 3)
            for global_name in self.output_names:
                self.event(registry, "global", global_name, "wl_output", 4)
            if self.layer_shell:
                self.event(registry, "global", 3, "river_layer_shell_v1", 1)
        elif interface == "wl_registry" and name == "bind" and self.objects[arguments["id"]] == "wl_output":
            self.event(arguments["id"], "name", self.output_names[arguments["name"]])
            self.event(arguments["id"], "done")
        elif interface == "wl_display" and name == "sync":
            callback = arguments["callback"]
            self.event(callback, "done", 1)
            self.event(1, "delete_id", callback)
        elif name == "get_xkb_binding":
            self.bindings[(arguments["keysym"], arguments["modifiers"])] = arguments["id"]
        elif name == "get_pointer_binding":
            self.pointer_bindings[(arguments["button"], arguments["modifiers"])] = arguments["id"]
        elif interface == "river_layer_shell_v1" and name == "get_output":
            assert arguments["output"] not in self.layer_outputs, "duplicate layer output"
            self.layer_outputs[arguments["output"]] = arguments["id"]
        elif interface == "river_layer_shell_v1" and name == "get_seat":
            assert arguments["seat"] not in self.layer_seats, "duplicate layer seat"
            self.layer_seats[arguments["seat"]] = arguments["id"]
        elif name == "get_node":
            self.nodes[object_id] = arguments["id"]
        return object_id, name, arguments

    def until(self, name):
        requests = []
        while True:
            request = self.request()
            requests.append(request)
            if request[1] == name:
                return requests

    def child(self, event):
        self.server_id += 1
        self.event(self.manager, event, self.server_id)
        return self.server_id

    def cycle(self):
        self.event(self.manager, "manage_start")
        requests = self.until("manage_finish")
        self.event(self.manager, "render_start")
        return requests + self.until("render_finish")

    def key(self, symbol, shift=False):
        self.event(self.bindings[(ord(symbol), 64 | int(shift))], "pressed")
        return self.cycle()


def has(requests, name, object_id=None, **arguments):
    return any(n == name and (object_id is None or obj == object_id)
               and all(args.get(k) == v for k, v in arguments.items())
               for obj, n, args in requests)


def main():
    peer = RiverPeer()
    try:
        while len([i for i in peer.objects.values() if i.startswith("river_")]) < 2:
            peer.request()
        left = peer.child("output")
        peer.event(left, "position", 0, 0)
        peer.event(left, "dimensions", 1920, 1080)
        right = peer.child("output")
        peer.event(right, "position", 1920, 0)
        peer.event(right, "dimensions", 1920, 1080)
        seat = peer.child("seat")
        peer.event(seat, "pointer_position", 100, 100)
        peer.cycle()
        first = peer.child("window")
        requests = peer.cycle()
        assert has(requests, "propose_dimensions", first, width=1920, height=1080)
        assert has(requests, "focus_window", seat, window=first)
        second = peer.child("window")
        requests = peer.cycle()
        assert has(requests, "propose_dimensions", first, width=960)
        assert has(requests, "propose_dimensions", second, width=960)
        # Tiled columns can be resized horizontally with Super + right mouse button.
        peer.event(seat, "pointer_enter", first)
        peer.event(peer.pointer_bindings[(0x111, 64)], "pressed")
        requests = peer.cycle()
        assert has(requests, "inform_resize_start", first)
        peer.event(seat, "op_delta", 200, 0)
        requests = peer.cycle()
        assert has(requests, "propose_dimensions", first, width=1160)
        assert has(requests, "set_position", peer.nodes[second], x=1160, y=0)
        peer.event(seat, "op_release")
        assert has(peer.cycle(), "inform_resize_end", first)
        # Floating does not consume a column, is centered and renders above tiles.
        peer.event(seat, "window_interaction", second)
        peer.cycle()
        requests = peer.key("v")
        assert has(requests, "set_tiled", second, edges=0)
        assert has(requests, "propose_dimensions", first, width=1160)
        assert has(requests, "propose_dimensions", second, width=1280, height=720)
        assert has(requests, "set_position", peer.nodes[second], x=320, y=180)
        assert [obj for obj, name, _ in requests if name == "place_top"][-1] == peer.nodes[second]
        # Focusing the tile must not cover the floating window.
        peer.event(seat, "window_interaction", first)
        requests = peer.cycle()
        assert [obj for obj, name, _ in requests if name == "place_top"][-1] == peer.nodes[second]
        peer.event(seat, "pointer_enter", second)
        peer.event(seat, "pointer_position", 1500, 800)
        peer.event(peer.pointer_bindings[(0x110, 64)], "pressed")
        requests = peer.cycle()
        assert has(requests, "op_start_pointer", seat)
        assert has(requests, "focus_window", seat, window=second)
        peer.event(seat, "op_delta", 50, 30)
        requests = peer.cycle()
        assert has(requests, "set_position", peer.nodes[second], x=370, y=210)
        peer.event(seat, "op_delta", 60, 40)
        requests = peer.cycle()
        assert has(requests, "set_position", peer.nodes[second], x=380, y=220)
        peer.event(seat, "op_release")
        assert has(peer.cycle(), "op_end", seat)
        peer.event(seat, "pointer_enter", second)
        peer.event(seat, "pointer_position", 1600, 900)
        peer.event(peer.pointer_bindings[(0x111, 64)], "pressed")
        requests = peer.cycle()
        assert has(requests, "inform_resize_start", second)
        peer.event(seat, "op_delta", 100, 50)
        requests = peer.cycle()
        assert has(requests, "propose_dimensions", second, width=1380, height=770)
        peer.event(seat, "op_release")
        requests = peer.cycle()
        assert has(requests, "inform_resize_end", second) and has(requests, "op_end", seat)
        # Client-side decorations can resize from the top-left, preserving the opposite corner.
        peer.event(second, "pointer_resize_requested", seat, 5)
        assert has(peer.cycle(), "op_start_pointer", seat)
        peer.event(seat, "op_delta", 20, 20)
        requests = peer.cycle()
        assert has(requests, "set_position", peer.nodes[second], x=400, y=240)
        assert has(requests, "propose_dimensions", second, width=1360, height=750)
        requests = peer.key("2")
        assert has(requests, "op_end", seat) and has(requests, "inform_resize_end", second)
        assert has(requests, "hide", second)
        requests = peer.key("1")
        assert has(requests, "set_position", peer.nodes[second], x=400, y=240)
        # Moving between workspaces preserves floating state and geometry.
        requests = peer.key("2", shift=True)
        assert has(requests, "hide", second)
        requests = peer.key("2")
        assert has(requests, "set_tiled", second, edges=0)
        assert has(requests, "set_position", peer.nodes[second], x=400, y=240)
        peer.key("1", shift=True)
        peer.key("1")
        requests = peer.key("v")
        assert has(requests, "set_tiled", second, edges=15)
        assert has(requests, "propose_dimensions", first, width=1160)
        requests = peer.key("v")
        assert has(requests, "propose_dimensions", second, width=1360, height=750)
        peer.key("v")
        # Mouse drags on tiled windows are deliberately ignored.
        peer.event(seat, "pointer_enter", second)
        peer.event(peer.pointer_bindings[(0x110, 64)], "pressed")
        assert not has(peer.cycle(), "op_start_pointer", seat)
        requests = peer.key("2")
        assert has(requests, "hide", first) and has(requests, "hide", second)
        assert has(requests, "clear_focus", seat)
        third = peer.child("window")
        requests = peer.cycle()
        assert has(requests, "propose_dimensions", third, width=1920)
        requests = peer.key("1")
        assert has(requests, "show", first) and has(requests, "show", second)
        assert has(requests, "hide", third)
        assert has(requests, "focus_window", seat, window=second)
        requests = peer.key("2", shift=True)
        assert has(requests, "hide", second)
        assert has(requests, "focus_window", seat, window=first)
        assert has(requests, "propose_dimensions", first, width=1160)
        requests = peer.key("2")
        assert has(requests, "show", second) and has(requests, "show", third)
        assert has(requests, "focus_window", seat, window=second)
        # Closing a hidden window must not change the active workspace's focus.
        peer.event(first, "closed")
        requests = peer.cycle()
        assert not has(requests, "focus_window") and not has(requests, "clear_focus")
        # A workspace switch on another monitor must leave the left side shown.
        peer.event(seat, "pointer_position", 2000, 100)
        peer.cycle()
        requests = peer.key("3")
        assert has(requests, "show", second) and has(requests, "show", third)
        assert has(requests, "clear_focus", seat)
        fourth = peer.child("window")
        peer.cycle()
        # Removing the monitor migrates its windows without showing workspace 3.
        peer.event(right, "removed")
        requests = peer.cycle()
        assert has(requests, "hide", fourth)
        peer.event(seat, "pointer_position", 100, 100)
        requests = peer.key("3")
        assert has(requests, "show", fourth)
        assert has(requests, "hide", second) and has(requests, "hide", third)
        assert has(requests, "focus_window", seat, window=fourth)
        # Losing the last output must clear focus and preserve workspace membership.
        peer.event(left, "removed")
        requests = peer.cycle()
        assert has(requests, "hide", fourth) and has(requests, "clear_focus", seat)
        restored = peer.child("output")
        peer.event(restored, "position", 0, 0)
        peer.event(restored, "dimensions", 1920, 1080)
        requests = peer.cycle()
        assert has(requests, "show", fourth)
        assert has(requests, "hide", second) and has(requests, "hide", third)
        assert has(requests, "focus_window", seat, window=fourth)
        # Closing a window mid-drag must end the operation without touching its dead object.
        peer.key("v")
        peer.event(fourth, "pointer_resize_requested", seat, 10)
        assert has(peer.cycle(), "inform_resize_start", fourth)
        peer.event(fourth, "closed")
        requests = peer.cycle()
        assert has(requests, "op_end", seat)
        assert not has(requests, "inform_resize_end", fourth)
        requests = peer.key("4")
        assert has(requests, "clear_focus", seat)
        # Session exit is available even on an empty workspace.
        peer.event(peer.bindings[(ord("m"), 64)], "pressed")
        peer.event(peer.manager, "manage_start")
        requests = peer.until("manage_finish")
        assert has(requests, "exit_session", peer.manager)
        print("Protocol checks passed: workspace visibility, focus, move, close, multiple outputs, unplug/replug, floating, pointer move/resize, cancellation, exit")
    finally:
        peer.close()


if __name__ == "__main__":
    main()
