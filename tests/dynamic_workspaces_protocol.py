#!/usr/bin/env python3
"""Extra workspaces come and go per monitor; the gaming workspace follows gaming_output."""
import tempfile
from pathlib import Path

from bar_client import BarClient
from river_protocol import RiverPeer, has

CTRL, SUPER = 4, 64


def main():
    with tempfile.TemporaryDirectory(prefix="mywm-dynamic-") as directory:
        config = Path(directory) / "config.toml"
        config.write_text(
            'workspace_outputs = ["DP-3", "HDMI-A-1"]\n'
            'gaming_output = "HDMI-A-1"\n'
            "game_app_id_prefixes = ['steam_app_']\n"
        )
        socket_path = Path(directory) / "ipc.sock"
        peer = RiverPeer(config=config, output_names={100: "DP-3", 101: "HDMI-A-1"},
                         ipc_socket=socket_path)
        try:
            while len([name for name in peer.objects.values() if name.startswith("river_")]) < 2:
                peer.request()
            main_output = peer.child("output")
            peer.event(main_output, "wl_output", 100)
            peer.event(main_output, "position", 0, 0)
            peer.event(main_output, "dimensions", 1920, 1080)
            side_output = peer.child("output")
            peer.event(side_output, "wl_output", 101)
            peer.event(side_output, "position", 1920, 0)
            peer.event(side_output, "dimensions", 1920, 1080)
            seat = peer.child("seat")
            peer.event(seat, "pointer_position", 100, 100)
            peer.cycle()
            peer.cycle()
            bar = BarClient(socket_path)

            def main_shows(active, entries):
                bar.expect(main_output, active, entries)

            def side_shows(active, entries):
                bar.expect(side_output, active, entries)

            # One fixed workspace per monitor, numbered in workspace_outputs order.
            main_shows(1, [(1, 0)])
            side_shows(2, [(2, 0)])

            # Super+n creates an extra workspace on the current monitor; repeating
            # it reuses the empty one, and leaving it removes it again.
            peer.key("n")
            main_shows(3, [(1, 0), (3, 0)])
            peer.key("n")
            main_shows(3, [(1, 0), (3, 0)])
            peer.key("1")
            main_shows(1, [(1, 0)])
            side_shows(2, [(2, 0)])

            # Super+Shift+n sends the focused window to a new workspace without following.
            window = peer.child("window")
            assert has(peer.cycle(), "show", window)
            requests = peer.key("n", shift=True)
            assert has(requests, "hide", window)
            main_shows(1, [(1, 0), (3, 1)])
            requests = peer.key("3")
            assert has(requests, "show", window) and has(requests, "focus_window", seat, window=window)
            # A shown extra stays while empty, and is dropped once left.
            peer.event(window, "closed")
            peer.cycle()
            main_shows(3, [(1, 0), (3, 0)])
            peer.key("1")
            main_shows(1, [(1, 0)])

            # Extras belong to the monitor they were created on and use the free numbers.
            peer.event(seat, "pointer_position", 2000, 100)
            peer.cycle()
            peer.key("n")
            side_shows(3, [(2, 0), (3, 0)])
            main_shows(1, [(1, 0)])
            peer.event(seat, "pointer_position", 100, 100)
            peer.cycle()
            peer.key("n")
            main_shows(4, [(1, 0), (4, 0)])
            # Ctrl+arrows cycle through the monitor's own workspaces only.
            peer.event(peer.bindings[(0xff53, SUPER | CTRL)], "pressed")
            peer.cycle()
            main_shows(1, [(1, 0)])
            peer.event(peer.bindings[(0xff53, SUPER | CTRL)], "pressed")
            peer.cycle()
            main_shows(1, [(1, 0)])
            # Workspaces that do not exist cannot be selected.
            peer.key("7")
            main_shows(1, [(1, 0)])
            side_shows(3, [(2, 0), (3, 0)])

            # The gaming workspace appears on gaming_output, wherever the game starts.
            game = peer.child("window")
            peer.cycle()
            peer.event(game, "app_id", "steam_app_1")
            peer.cycle()
            # The monitor's empty extra survives underneath, and is shown again afterwards.
            side_shows(0, [(2, 0), (3, 0), (0, 1)])
            main_shows(1, [(1, 0)])
            peer.event(game, "closed")
            peer.cycle()
            side_shows(3, [(2, 0), (3, 0)])
            print("Dynamic workspaces passed: create/reuse/prune, move to new, per-monitor numbering, cycling, gaming output")
        finally:
            peer.close()


if __name__ == "__main__":
    main()
