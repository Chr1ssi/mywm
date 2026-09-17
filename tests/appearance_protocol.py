#!/usr/bin/env python3
"""Default decoration geometry, border colors and clipping in actual requests."""
from river_protocol import ROOT, RiverPeer, has


def main():
    peer = RiverPeer(config=ROOT / "config/mywm.toml", layer_shell=True)
    try:
        while "river_layer_shell_v1" not in peer.objects.values():
            peer.request()
        output = peer.child("output")
        peer.event(output, "position", 0, 0)
        peer.event(output, "dimensions", 1920, 1080)
        seat = peer.child("seat")
        peer.event(seat, "pointer_position", 100, 100)
        peer.cycle()
        first = peer.child("window")
        requests = peer.cycle()
        assert has(requests, "use_ssd", first)
        assert has(requests, "propose_dimensions", first, width=1900, height=1060)
        assert has(requests, "set_position", peer.nodes[first], x=10, y=10)
        assert has(requests, "set_borders", first, width=2, r=0x89898989, g=0xb4b4b4b4, b=0xfafafafa, a=0xffffffff)
        assert has(requests, "set_clip_box", first, x=-2, y=-2, width=1904, height=1064)
        peer.event(peer.layer_outputs[output], "non_exclusive_area", 0, 40, 1920, 1040)
        requests = peer.cycle()
        assert has(requests, "propose_dimensions", first, width=1900, height=1020)
        assert has(requests, "set_position", peer.nodes[first], x=10, y=50)
        peer.event(peer.layer_outputs[output], "non_exclusive_area", 0, 0, 1920, 1080)
        second = peer.child("window")
        requests = peer.cycle()
        assert has(requests, "propose_dimensions", first, width=944, height=1060)
        assert has(requests, "set_position", peer.nodes[second], x=966, y=10)
        assert has(requests, "set_borders", first, width=2, r=0x45454545)
        third = peer.child("window")
        requests = peer.cycle()
        assert has(requests, "hide", first)
        assert has(requests, "set_position", peer.nodes[second], x=10, y=10)
        assert has(requests, "set_position", peer.nodes[third], x=966, y=10)
        assert has(requests, "set_clip_box", third, x=-2, y=-2, width=948, height=1064)
        requests = peer.key("v")
        assert has(requests, "use_ssd", third)
        assert has(requests, "propose_dimensions", third, width=1276, height=716)
        assert has(requests, "set_position", peer.nodes[third], x=322, y=182)
        assert has(requests, "set_borders", third, width=2, r=0x89898989)
        peer.event(peer.layer_seats[seat], "focus_exclusive")
        requests = peer.cycle()
        assert has(requests, "set_borders", third, r=0x45454545)
        peer.event(peer.layer_seats[seat], "focus_none")
        requests = peer.cycle()
        assert has(requests, "set_borders", third, r=0x89898989)
        print("Appearance checks passed: gaps, content sizes, border clipping, floating and shell focus colors")
    finally:
        peer.close()


if __name__ == "__main__":
    main()
