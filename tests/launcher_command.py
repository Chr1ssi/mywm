#!/usr/bin/env python3
"""Verify the real launcher key action forwards configuration without shell parsing."""
import json
from pathlib import Path
import sys
import tempfile
import time

from river_protocol import RiverPeer


with tempfile.TemporaryDirectory(prefix="mywm-command-") as directory:
    root = Path(directory)
    result = root / "result.json"
    probe = root / "probe.py"
    probe.write_text("import json, os, pathlib, sys\n"
                     "pathlib.Path(sys.argv[1]).write_text(json.dumps(dict(os.environ)))\n")
    config = root / "config.toml"
    config.write_text(
        "launcher = " + json.dumps([sys.executable, str(probe), str(result)]) + "\n"
        'terminal = ["kitty", "--title", "two words"]\n'
        '[appearance]\nbackground = "#123456"\nactive_border = "#abcdef"\n')
    peer = RiverPeer(config)
    try:
        while len([i for i in peer.objects.values() if i.startswith("river_")]) < 2:
            peer.request()
        output = peer.child("output")
        peer.event(output, "position", 0, 0)
        peer.event(output, "dimensions", 1920, 1080)
        seat = peer.child("seat")
        peer.event(seat, "pointer_position", 123, 456)
        peer.cycle()
        peer.key(" ")
        deadline = time.monotonic() + 5
        while not result.exists() and time.monotonic() < deadline:
            time.sleep(0.02)
        env = json.loads(result.read_text())
        assert env["MYWM_COLOR_BACKGROUND"] == "#123456"
        assert env["MYWM_COLOR_ACCENT"] == "#abcdef"
        assert env["MYWM_TERMINAL_COUNT"] == "3"
        assert [env[f"MYWM_TERMINAL_{i}"] for i in range(3)] == ["kitty", "--title", "two words"]
        assert (env["MYWM_LAUNCHER_X"], env["MYWM_LAUNCHER_Y"]) == ("123", "456")
        print("Launcher key action passed: shared palette, terminal argv, monitor position")
    finally:
        peer.close()
