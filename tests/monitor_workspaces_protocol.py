#!/usr/bin/env python3
"""Global workspace routing by connector, cross-output moves and hotplug."""
from pathlib import Path
import tempfile
from river_protocol import ROOT, RiverPeer, has

with tempfile.TemporaryDirectory(prefix='mywm-monitors-') as directory:
    config = Path(directory) / 'config.toml'
    config.write_text((ROOT/'tests/fixtures/plain.toml').read_text() + '''
[workspace_outputs]
DP-1 = [1, 2, 3]
HDMI-A-1 = [4, 5, 6]
DP-3 = [7, 8, 9]
[[rules]]
app_id = "test.rule"
workspace = 5
''')
    peer = RiverPeer(config=config, output_names={100: 'DP-3', 101: 'HDMI-A-1', 102: 'DP-1'})
    try:
        while len([i for i in peer.objects.values() if i.startswith('river_')]) < 2:
            peer.request()
        def output(global_name, x, y, width, height):
            obj = peer.child('output')
            peer.event(obj, 'wl_output', global_name)
            peer.event(obj, 'position', x, y)
            peer.event(obj, 'dimensions', width, height)
            return obj
        main = output(100, 0, 1080, 2560, 1440)
        top = output(101, 0, 0, 2560, 1080)
        portrait = output(102, 2560, 0, 1440, 2560)
        seat = peer.child('seat')
        peer.event(seat, 'pointer_position', 100, 1200)
        peer.cycle()
        peer.cycle()  # output names arrive on wl_output independently of River events
        first = peer.child('window')
        requests = peer.cycle()
        assert has(requests, 'set_position', peer.nodes[first], x=0, y=1080)
        assert has(requests, 'propose_dimensions', first, width=2560, height=1440)
        # Main monitor starts on global workspace 7, not 1.
        requests = peer.key('8')
        assert has(requests, 'hide', first)
        assert has(peer.key('7'), 'show', first)
        # Move to DP-1 workspace 2 without following. Original monitor stays on 7.
        requests = peer.key('2', shift=True)
        assert has(requests, 'hide', first)
        assert has(requests, 'clear_focus', seat)
        requests = peer.key('2')
        assert has(requests, 'pointer_warp', seat, x=3280, y=1280)
        assert has(requests, 'set_position', peer.nodes[first], x=2560, y=0)
        assert has(requests, 'propose_dimensions', first, width=1440, height=2560)
        assert has(requests, 'focus_window', seat, window=first)
        # New windows follow the selected monitor even without physical mouse motion.
        second = peer.child('window')
        requests = peer.cycle()
        assert has(requests, 'propose_dimensions', second, width=720)
        # Moving floating windows to the upper monitor recenters and resizes them.
        peer.key('v')
        peer.key('4', shift=True)
        requests = peer.key('4')
        assert has(requests, 'set_position', peer.nodes[second], x=427, y=180)
        assert has(requests, 'propose_dimensions', second, width=1706, height=720)
        # Rules also use global workspace ownership.
        ruled = peer.child('window')
        peer.event(ruled, 'app_id', 'test.rule')
        assert has(peer.cycle(), 'hide', ruled)
        requests = peer.key('5')
        assert has(requests, 'show', ruled)
        assert has(requests, 'set_position', peer.nodes[ruled], x=0, y=0)
        # DP-1's workspace temporarily moves to the remaining monitor; restores on replug.
        registry = next(obj for obj, interface in peer.objects.items() if interface == 'wl_registry')
        peer.event(registry, 'global_remove', 102)
        peer.event(portrait, 'removed')
        peer.cycle()
        requests = peer.key('2')
        assert has(requests, 'show', first)
        assert has(requests, 'set_position', peer.nodes[first], x=0, y=1080)
        peer.output_names[103] = 'DP-1'
        peer.event(registry, 'global', 103, 'wl_output', 4)
        portrait = output(103, 2560, 0, 1440, 2560)
        peer.cycle()
        peer.cycle()
        requests = peer.key('2')
        assert has(requests, 'set_position', peer.nodes[first], x=2560, y=0)
        assert has(requests, 'propose_dimensions', first, width=1440, height=2560)
        print('Monitor workspaces passed: connector mapping, global keys/rules, cross-monitor tiled/floating moves, pointer focus, unplug/replug')
    finally:
        peer.close()
