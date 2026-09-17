#!/usr/bin/env python3
"""Fullscreen negotiation, layout restoration, workspaces and output migration."""
from river_protocol import RiverPeer, has


def main():
    peer = RiverPeer(layer_shell=True)
    try:
        while 'river_layer_shell_v1' not in peer.objects.values():
            peer.request()
        output = peer.child('output')
        peer.event(output, 'position', 0, 0)
        peer.event(output, 'dimensions', 1920, 1080)
        seat = peer.child('seat')
        peer.event(seat, 'pointer_position', 100, 100)
        peer.cycle()
        peer.event(peer.layer_outputs[output], 'non_exclusive_area', 0, 36, 1920, 1044)
        first = peer.child('window')
        peer.cycle()
        second = peer.child('window')
        normal = peer.cycle()
        peer.event(second, 'fullscreen_requested', 0)
        requests = peer.cycle()
        assert has(requests, 'fullscreen', second, output=output)
        assert has(requests, 'inform_fullscreen', second)
        assert has(requests, 'show', second)
        assert not has(requests, 'propose_dimensions', second)
        assert not has(peer.cycle(), 'fullscreen', second), 'do not renegotiate unchanged fullscreen'
        requests = peer.key('2')
        assert has(requests, 'exit_fullscreen', second)
        assert has(requests, 'hide', second)
        requests = peer.key('1')
        assert has(requests, 'fullscreen', second, output=output)
        peer.event(second, 'exit_fullscreen_requested')
        requests = peer.cycle()
        assert has(requests, 'exit_fullscreen', second)
        assert has(requests, 'inform_not_fullscreen', second)
        for obj, name, args in normal:
            if (obj == second and name in ('propose_dimensions', 'set_borders', 'set_clip_box')) or (obj == peer.nodes[second] and name == 'set_position'):
                assert has(requests, name, obj, **args), (name, args)
        floating = peer.key('v')
        peer.event(second, 'fullscreen_requested', output)
        assert has(peer.cycle(), 'fullscreen', second, output=output)
        peer.event(second, 'exit_fullscreen_requested')
        requests = peer.cycle()
        for obj, name, args in floating:
            if obj == second and name == 'propose_dimensions':
                assert has(requests, name, obj, **args)
        other = peer.child('output')
        peer.event(other, 'position', 1920, 0)
        peer.event(other, 'dimensions', 1440, 2560)
        peer.cycle()
        # App output hints do not override workspace ownership.
        peer.event(second, 'fullscreen_requested', other)
        assert has(peer.cycle(), 'fullscreen', second, output=output)
        peer.event(output, 'removed')
        assert has(peer.cycle(), 'fullscreen', second, output=other)
        print('Fullscreen protocol passed: enter/exit, stable state, tiled/floating restore, workspace switch, output hint and unplug')
    finally:
        peer.close()


if __name__ == '__main__':
    main()
