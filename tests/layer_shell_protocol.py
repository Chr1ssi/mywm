#!/usr/bin/env python3
"""Layer-shell work areas and launcher keyboard-focus integration."""
from river_protocol import RiverPeer, has


def main():
    peer = RiverPeer(layer_shell=True)
    try:
        while "river_layer_shell_v1" not in peer.objects.values():
            peer.request()
        output = peer.child("output")
        peer.event(output, "position", 100, 200)
        peer.event(output, "dimensions", 1920, 1080)
        seat = peer.child("seat")
        peer.event(seat, "pointer_position", 200, 300)
        requests = peer.cycle()
        layer_output = peer.layer_outputs[output]
        layer_seat = peer.layer_seats[seat]
        assert has(requests, "set_default", layer_output)
        peer.event(layer_output, "non_exclusive_area", 100, 240, 1920, 1000)
        window = peer.child("window")
        requests = peer.cycle()
        assert has(requests, "propose_dimensions", window, width=1920, height=1000)
        assert has(requests, "set_position", peer.nodes[window], x=100, y=240)
        assert has(requests, "set_clip_box", window, x=0, y=0, width=1920, height=1000)
        requests = peer.key("v")
        assert has(requests, "propose_dimensions", window, width=1280, height=666)
        assert has(requests, "set_position", peer.nodes[window], x=420, y=407)
        # Drag constraints must use the usable area, not the entire monitor.
        peer.event(window, "pointer_move_requested", seat)
        assert has(peer.cycle(), "op_start_pointer", seat)
        peer.event(seat, "op_delta", -9999, -9999)
        requests = peer.cycle()
        assert has(requests, "set_position", peer.nodes[window], x=100, y=240)
        # A bar geometry update terminates a drag based on the old origin.
        peer.event(layer_output, "non_exclusive_area", 100, 260, 1920, 980)
        requests = peer.cycle()
        assert has(requests, "op_end", seat)
        peer.key("v")
        # Covering all space must hide windows rather than proposing zero dimensions.
        peer.event(layer_output, "non_exclusive_area", 100, 1280, 1920, 0)
        requests = peer.cycle()
        assert has(requests, "hide", window) and has(requests, "clear_focus", seat)
        assert not has(requests, "propose_dimensions", window)
        peer.event(layer_output, "non_exclusive_area", 100, 200, 1920, 1080)
        requests = peer.cycle()
        assert has(requests, "show", window) and has(requests, "focus_window", seat, window=window)
        # An exclusive launcher owns focus even when the user changes workspaces.
        peer.event(layer_seat, "focus_exclusive")
        requests = peer.key("2")
        assert not has(requests, "focus_window") and not has(requests, "clear_focus")
        peer.event(layer_seat, "focus_none")
        assert has(peer.cycle(), "clear_focus", seat)
        peer.key("1")
        # An on-demand panel must not lose focus to incidental work-area updates.
        peer.event(layer_seat, "focus_non_exclusive")
        requests = peer.cycle()
        assert not has(requests, "focus_window")
        peer.event(layer_output, "non_exclusive_area", 100, 240, 1920, 1040)
        requests = peer.cycle()
        assert not has(requests, "focus_window") and not has(requests, "clear_focus")
        # Explicit interaction with a window can leave non-exclusive shell focus.
        peer.event(seat, "window_interaction", window)
        assert has(peer.cycle(), "focus_window", seat, window=window)
        peer.event(layer_seat, "focus_none")
        peer.cycle()
        # Newly mapped apps must not steal focus in the launcher's grant sequence.
        peer.event(layer_seat, "focus_non_exclusive")
        new_window = peer.child("window")
        requests = peer.cycle()
        assert not has(requests, "focus_window")
        peer.event(layer_seat, "focus_none")
        assert has(peer.cycle(), "focus_window", seat, window=new_window)
        # Extension objects must be released with their output/seat lifetimes.
        peer.event(output, "removed")
        requests = peer.cycle()
        assert has(requests, "destroy", layer_output)
        peer.event(seat, "removed")
        requests = peer.cycle()
        assert has(requests, "destroy", layer_seat)
        print("Layer-shell protocol checks passed: work areas, floating, launcher focus, restoration, cleanup")
    finally:
        peer.close()


if __name__ == "__main__":
    main()
