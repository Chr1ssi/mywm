"""Minimal client of the bar protocol for protocol tests."""
import socket

from smoke_support import wait_for


class BarClient:
    def __init__(self, path):
        wait_for(lambda: path.exists())
        self.socket = socket.socket(socket.AF_UNIX)
        self.socket.connect(str(path))
        self.socket.settimeout(0.05)
        self.pending = b""
        self.outputs = {}

    def poll(self):
        """Read everything available; `outputs` maps output id to (active, workspaces)."""
        try:
            self.pending += self.socket.recv(65536)
        except socket.timeout:
            pass
        *lines, self.pending = self.pending.split(b"\n")
        for line in lines:
            text = line.decode()
            if text.startswith("v1 state "):
                self.outputs = {}
                for entry in filter(None, text.split(" ", 2)[2].split(";")):
                    fields = entry.split(",")
                    self.outputs[int(fields[0])] = (
                        int(fields[5]),
                        [tuple(map(int, item.split(":"))) for item in fields[8].split("|")],
                    )

    def send(self, command):
        self.socket.sendall(f"v1 {command}\n".encode())

    def expect(self, output, active, workspaces, drive=lambda: None):
        """Wait until `output` shows `active` with `workspaces` [(number, occupied)]."""
        def check():
            drive()
            self.poll()
            return self.outputs.get(output) == (active, workspaces)
        try:
            wait_for(check)
        except AssertionError:
            raise AssertionError(f"expected {(active, workspaces)}, got {self.outputs.get(output)}")
