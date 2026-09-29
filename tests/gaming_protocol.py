#!/usr/bin/env python3
"""Dynamic gaming workspace: late app IDs, non-game exclusion and disappearing."""
import tempfile
from pathlib import Path

from river_protocol import RiverPeer, has
from bar_client import BarClient


def main():
    with tempfile.TemporaryDirectory(prefix="mywm-gaming-") as directory:
        config = Path(directory) / "config.toml"
        config.write_text("game_app_id_prefixes = ['steam_app_']\n")
        socket_path = Path(directory) / "ipc.sock"
        peer = RiverPeer(config=config, ipc_socket=socket_path)
        try:
            while len([name for name in peer.objects.values() if name.startswith("river_")]) < 2:
                peer.request()
            output = peer.child("output")
            peer.event(output, "position", 0, 0)
            peer.event(output, "dimensions", 1920, 1080)
            seat = peer.child("seat")
            peer.event(seat, "pointer_position", 100, 100)
            peer.cycle()
            bar = BarClient(socket_path)

            def workspaces(active, entries):
                bar.expect(output, active, entries)

            # Without a running game there is no gaming workspace.
            workspaces(1, [(1, 0)])

            game = peer.child("window")
            assert has(peer.cycle(), "show", game)
            peer.event(game, "app_id", "steam_app_394360")
            requests = peer.cycle()
            assert has(requests, "focus_window", seat, window=game)
            workspaces(0, [(1, 0), (0, 1)])

            # Regular windows never start on the gaming workspace.
            regular = peer.child("window")
            requests = peer.cycle()
            assert has(requests, "hide", game)
            assert has(requests, "show", regular)
            assert has(requests, "focus_window", seat, window=regular)
            workspaces(1, [(1, 1), (0, 1)])

            # The workspace can be selected again (as the bar does) and vanishes
            # with the game, returning to where the user came from.
            bar.send(f"workspace {output} 0")
            bar.expect(output, 0, [(1, 1), (0, 1)], drive=peer.cycle)
            peer.event(game, "closed")
            requests = peer.cycle()
            assert has(requests, "focus_window", seat, window=regular)
            workspaces(1, [(1, 1)])
            print("Gaming workspace passed: late game routing, non-game exclusion, appears and disappears with the game")
        finally:
            peer.close()


if __name__ == "__main__":
    main()
