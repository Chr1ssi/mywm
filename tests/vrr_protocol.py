#!/usr/bin/env python3
"""Adaptive sync follows visible fullscreen games on the configured output."""
import os
import tempfile
from pathlib import Path

from river_protocol import RiverPeer
from smoke_support import wait_for


def main():
    with tempfile.TemporaryDirectory(prefix="mywm-vrr-") as directory:
        base = Path(directory)
        log = base / "vrr.log"
        command = base / "vrr-command"
        command.write_text(
            "#!/bin/sh\n"
            "log=$1\n"
            "shift\n"
            "printf '%s\\n' \"$*\" >> \"$log\"\n"
        )
        command.chmod(0o755)
        config = base / "config.toml"
        config.write_text(
            "workspaces = 2\n"
            "game_app_id_prefixes = ['steam_app_']\n"
            "[vrr]\n"
            "enabled = true\n"
            "output = 'DP-3'\n"
            f"command = ['{command}', '{log}']\n"
        )
        peer = RiverPeer(config=config, output_names={100: "DP-3"})
        try:
            while len([name for name in peer.objects.values() if name.startswith("river_")]) < 2:
                peer.request()
            output = peer.child("output")
            peer.event(output, "wl_output", 100)
            peer.event(output, "position", 0, 0)
            peer.event(output, "dimensions", 1920, 1080)
            seat = peer.child("seat")
            peer.event(seat, "pointer_position", 100, 100)
            peer.cycle()

            game = peer.child("window")
            peer.event(game, "app_id", "steam_app_123")
            peer.cycle()
            peer.event(game, "fullscreen_requested", 0)
            peer.cycle()
            peer.key("2")

            changes = wait_for(
                lambda: log.read_text().splitlines()
                if log.exists() and len(log.read_text().splitlines()) == 2
                else None
            )
            assert changes == [
                "--output DP-3 --adaptive-sync enabled",
                "--output DP-3 --adaptive-sync disabled",
            ], changes
            print("VRR protocol passed: desktop off, fullscreen game on, hidden game off")
        finally:
            peer.close()


if __name__ == "__main__":
    main()
