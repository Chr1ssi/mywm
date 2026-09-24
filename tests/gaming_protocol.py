#!/usr/bin/env python3
"""Gaming workspace routing, late app IDs and non-game exclusion."""
import tempfile
from pathlib import Path

from river_protocol import RiverPeer, has


def main():
    with tempfile.TemporaryDirectory(prefix="mywm-gaming-") as directory:
        config = Path(directory) / "config.toml"
        config.write_text(
            "workspaces = 3\n"
            "gaming_workspace = 3\n"
            "game_app_id_prefixes = ['steam_app_']\n"
        )
        peer = RiverPeer(config=config)
        try:
            while len([name for name in peer.objects.values() if name.startswith("river_")]) < 2:
                peer.request()
            output = peer.child("output")
            peer.event(output, "position", 0, 0)
            peer.event(output, "dimensions", 1920, 1080)
            seat = peer.child("seat")
            peer.event(seat, "pointer_position", 100, 100)
            peer.cycle()

            game = peer.child("window")
            assert has(peer.cycle(), "show", game)
            peer.event(game, "app_id", "steam_app_394360")
            requests = peer.cycle()
            assert has(requests, "focus_window", seat, window=game)

            regular = peer.child("window")
            requests = peer.cycle()
            assert has(requests, "hide", game)
            assert has(requests, "show", regular)
            assert has(requests, "focus_window", seat, window=regular)
            print("Gaming workspace passed: late game routing and non-game exclusion")
        finally:
            peer.close()


if __name__ == "__main__":
    main()
