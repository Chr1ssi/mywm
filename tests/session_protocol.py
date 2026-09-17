#!/usr/bin/env python3
"""Check that locked sessions disable WM bindings and reject queued exit actions."""
from river_protocol import RiverPeer, has

peer = RiverPeer()
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
    assert (ord("w"), 65) in peer.bindings, "Super+Shift+W not registered"
    assert (0xff1b, 64) in peer.bindings, "Super+Escape not registered"
    peer.event(peer.manager, "session_locked")
    requests = peer.cycle()
    for binding in [*peer.bindings.values(), *peer.pointer_bindings.values()]:
        assert has(requests, "disable", binding)
    assert not has(peer.key("m"), "exit_session"), "Queued exit ran while locked"
    peer.event(peer.manager, "session_unlocked")
    requests = peer.cycle()
    for binding in [*peer.bindings.values(), *peer.pointer_bindings.values()]:
        assert has(requests, "enable", binding)
    assert has(peer.key("m"), "exit_session")
    print("Session protocol passed: Super+Escape, disabled key/mouse bindings, blocked exit, restored bindings")
finally:
    peer.close()
