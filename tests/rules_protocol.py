#!/usr/bin/env python3
"""Integration checks for first-placement rules and parent/dialog inheritance."""
from pathlib import Path
import tempfile
from river_protocol import RiverPeer, has


def main():
    with tempfile.TemporaryDirectory(prefix="mywm-rules-") as directory:
        config = Path(directory) / "config.toml"
        config.write_text('''
workspaces = 3
[appearance]
gaps_inner = 0
gaps_outer = 0
border_width = 0
[[rules]]
app_id = "test.background"
workspace = 2
[[rules]]
app_id = "test.float"
floating = true
[[rules]]
app_id = "test.force-tile"
dialog = true
floating = false
[[rules]]
app_id = "test.override"
workspace = 3
''')
        peer = RiverPeer(config)
        try:
            while len([i for i in peer.objects.values() if i.startswith("river_")]) < 2:
                peer.request()
            left = peer.child("output")
            peer.event(left, "position", 0, 0)
            peer.event(left, "dimensions", 1920, 1080)
            right = peer.child("output")
            peer.event(right, "position", 1920, 0)
            peer.event(right, "dimensions", 1280, 720)
            seat = peer.child("seat")
            peer.event(seat, "pointer_position", 100, 100)
            peer.cycle()
            anchor = peer.child("window")
            peer.cycle()
            background = peer.child("window")
            peer.event(background, "app_id", "test.background")
            requests = peer.cycle()
            assert has(requests, "hide", background)
            assert not has(requests, "focus_window") and not has(requests, "clear_focus")
            # Parent placement wins over the pointer's monitor, including hidden workspaces.
            peer.event(seat, "pointer_position", 2000, 100)
            dialog = peer.child("window")
            peer.event(dialog, "parent", background)
            requests = peer.cycle()
            assert has(requests, "hide", dialog)
            assert not has(requests, "focus_window")
            peer.event(seat, "pointer_position", 100, 100)
            requests = peer.key("2")
            assert has(requests, "show", background) and has(requests, "show", dialog)
            assert has(requests, "set_tiled", dialog, edges=0)
            assert has(requests, "propose_dimensions", dialog, width=1280, height=720)
            assert has(requests, "set_position", peer.nodes[dialog], x=320, y=180)
            # Explicit dialog rules override automatic floating; workspace rules override inheritance.
            forced = peer.child("window")
            peer.event(forced, "app_id", "test.force-tile")
            peer.event(forced, "parent", background)
            requests = peer.cycle()
            assert has(requests, "set_tiled", forced, edges=15)
            override = peer.child("window")
            peer.event(override, "app_id", "test.override")
            peer.event(override, "parent", background)
            requests = peer.cycle()
            assert has(requests, "hide", override)
            assert not has(requests, "focus_window")
            requests = peer.key("3")
            assert has(requests, "show", override)
            assert has(requests, "set_tiled", override, edges=0)
            # Manual changes survive later app_id/title events and unrelated manage cycles.
            floating = peer.child("window")
            peer.event(floating, "app_id", "test.float")
            assert has(peer.cycle(), "set_tiled", floating, edges=0)
            assert has(peer.key("v"), "set_tiled", floating, edges=15)
            peer.event(floating, "app_id", "test.float")
            peer.event(floating, "title", "Changed title")
            assert has(peer.cycle(), "set_tiled", floating, edges=15)
            peer.key("1", shift=True)
            peer.event(floating, "app_id", "test.background")
            assert has(peer.cycle(), "hide", floating)
            requests = peer.key("1")
            assert has(requests, "show", floating)
            # Resolve parents before children even when announcement order is reversed.
            child_first = peer.child("window")
            parent_second = peer.child("window")
            peer.event(child_first, "parent", parent_second)
            peer.event(parent_second, "app_id", "test.background")
            requests = peer.cycle()
            assert has(requests, "hide", child_first) and has(requests, "hide", parent_second)
            assert not has(requests, "focus_window")
            requests = peer.key("2")
            assert has(requests, "show", child_first) and has(requests, "show", parent_second)
            assert has(requests, "set_tiled", child_first, edges=0)
            assert has(requests, "focus_window", seat, window=child_first)
            print("Rules protocol checks passed: background placement, dialog inheritance, overrides, one-shot rules, parent ordering")
        finally:
            peer.close()


if __name__ == "__main__":
    main()
