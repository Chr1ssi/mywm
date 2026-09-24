#!/usr/bin/env python3
"""Hot reload replaces bindings atomically and rejects invalid configuration."""
from pathlib import Path
import tempfile

from river_protocol import RiverPeer, has


with tempfile.TemporaryDirectory(prefix="mywm-reload-") as directory:
    config = Path(directory) / "config.toml"
    config.write_text("")
    peer = RiverPeer(config=config)
    try:
        while len([i for i in peer.objects.values() if i.startswith("river_")]) < 2:
            peer.request()
        output = peer.child("output")
        peer.event(output, "position", 0, 0)
        peer.event(output, "dimensions", 1920, 1080)
        seat = peer.child("seat")
        peer.event(seat, "pointer_position", 100, 100)
        peer.cycle()
        peer.cycle()

        reload_key = (ord("r"), 65)  # Super+Shift
        assert reload_key in peer.bindings, peer.bindings
        assert (ord("b"), 64) not in peer.bindings

        config.write_text(
            "[program_bindings.browser]\n"
            "keys = ['Super+b']\n"
            "command = ['firefox']\n"
        )
        peer.event(peer.bindings[reload_key], "pressed")
        requests = peer.cycle()
        assert has(requests, "destroy"), "old bindings were not destroyed"
        assert (ord("b"), 64) in peer.bindings, "new binding was not installed"

        active_reload = peer.bindings[reload_key]
        active_browser = peer.bindings[(ord("b"), 64)]
        config.write_text("[bindings]\nunknown = []\n")
        peer.event(active_reload, "pressed")
        requests = peer.cycle()
        assert not has(requests, "destroy", active_reload)
        assert peer.bindings[(ord("b"), 64)] == active_browser
        print("Reload protocol passed: live bindings and atomic invalid-config rejection")
    finally:
        peer.close()
