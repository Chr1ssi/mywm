#!/usr/bin/env python3
"""Focus follows the pointer, but only when it actually moves."""
from river_protocol import RiverPeer, has


def main():
    peer = RiverPeer()
    try:
        while len([n for n in peer.objects.values() if n.startswith("river_")]) < 2:
            peer.request()
        output = peer.child("output")
        peer.event(output, "position", 0, 0)
        peer.event(output, "dimensions", 1920, 1080)
        seat = peer.child("seat")
        peer.event(seat, "pointer_position", 100, 100)
        peer.cycle()
        first = peer.child("window")
        peer.cycle()
        second = peer.child("window")
        assert has(peer.cycle(), "focus_window", seat, window=second)
        # A window scrolling under a resting pointer must not take focus.
        peer.event(seat, "pointer_enter", first)
        assert not has(peer.cycle(), "focus_window")
        # Moving the pointer over it does.
        peer.event(seat, "pointer_position", 120, 110)
        assert has(peer.cycle(), "focus_window", seat, window=first)
        # Motion order relative to the enter event does not matter.
        peer.event(seat, "pointer_position", 1500, 500)
        peer.event(seat, "pointer_enter", second)
        assert has(peer.cycle(), "focus_window", seat, window=second)
        print("Hover focus passed: needs motion, either event order")
    finally:
        peer.close()


if __name__ == "__main__":
    main()
