#!/usr/bin/env python3
"""Exercise real session locking only inside an isolated headless compositor."""
import os
from pathlib import Path
import signal
import socket
import subprocess
import tempfile
import time

from smoke_support import ROOT, wait_for


def main():
    with tempfile.TemporaryDirectory(prefix="mywm-lock-") as directory:
        base = Path(directory)
        runtime = base / "runtime"
        runtime.mkdir(mode=0o700)
        config = base / "config.toml"
        config.write_text('[idle]\nlock_after_seconds = 1\nmonitor_off_after_seconds = 2\n')
        env = dict(os.environ, XDG_RUNTIME_DIR=str(runtime), XDG_CONFIG_HOME=str(base / "config"),
                   WLR_BACKENDS="headless", WLR_HEADLESS_OUTPUTS="2", WLR_RENDERER="pixman",
                   MYWM_CONFIG=str(config), MYWM_SOCKET=str(runtime / "control.sock"))
        for key in ["WAYLAND_DISPLAY", "WAYLAND_SOCKET", "DISPLAY"]:
            env.pop(key, None)
        # Standby calls are recorded; the host's monitor power is never touched.
        mock_bin = base / "bin"
        mock_bin.mkdir()
        power_log = base / "power.log"
        mock = mock_bin / "wlopm"
        mock.write_text("#!/usr/bin/python3\nimport sys\nfrom pathlib import Path\n" +
                        f"with Path({str(power_log)!r}).open('a') as f: f.write(' '.join(sys.argv[1:]) + '\\n')\n")
        mock.chmod(0o755)
        env["PATH"] = str(mock_bin) + ":" + env["PATH"]
        idle = None
        with (base / "session.log").open("w+") as log:
            river = subprocess.Popen(["river", "-no-xwayland", "-c", "exec " + str(ROOT / "target/debug/mywm")],
                                     env=env, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
            try:
                wait_for(lambda: Path(env["MYWM_SOCKET"]).exists())
                env["WAYLAND_DISPLAY"] = next(p.name for p in runtime.glob("wayland-*") if p.is_socket())
                wm = wait_for(lambda: Path(f"/proc/{river.pid}/task/{river.pid}/children").read_text().split())
                wm_pid = int(wm[0])

                def lockers():
                    children = Path(f"/proc/{wm_pid}/task/{wm_pid}/children").read_text().split()
                    return [int(pid) for pid in children if Path(f"/proc/{pid}/comm").read_text().strip() == "swaylock"]

                def status():
                    with socket.socket(socket.AF_UNIX) as client:
                        client.settimeout(3)
                        client.connect(env["MYWM_SOCKET"])
                        result = b""
                        while b"v1 locked" not in result:
                            result += client.recv(4096)
                        return result

                def request_lock():
                    subprocess.run([str(ROOT / "target/debug/mywm"), "--lock"], env=env,
                                   check=True, timeout=10, stdout=log, stderr=subprocess.STDOUT)
                request_lock()
                assert b"v1 locked 1" in status()
                original = lockers()
                assert len(original) == 1
                request_lock()
                assert lockers() == original, "Duplicate locker"
                with socket.socket(socket.AF_UNIX) as client:
                    client.settimeout(3)
                    client.connect(env["MYWM_SOCKET"])
                    client.recv(4096)
                    client.sendall(b"v1 logout\n")
                    assert b"v1 error" in client.recv(4096), "Logout permitted while locked"
                if os.environ.get("MYWM_LOCK_SCREENSHOT"):
                    subprocess.run(["grim", os.environ["MYWM_LOCK_SCREENSHOT"]], env=env, check=True)
                # Test-only unlock of exactly the locker owned by this compositor.
                assert f"PPid:\t{wm_pid}\n" in Path(f"/proc/{original[0]}/status").read_text()
                os.kill(original[0], signal.SIGUSR1)
                wait_for(lambda: b"v1 locked 0" in status())
                idle = subprocess.Popen([str(ROOT / "target/debug/mywm"), "--idle"], env=env,
                                        stdout=log, stderr=subprocess.STDOUT)
                wait_for(lambda: b"v1 locked 1" in status())
                wait_for(lambda: power_log.exists() and "--off *" in power_log.read_text())
                idle.terminate()
                assert idle.wait(timeout=5) == 0
                wait_for(lambda: "--on *" in power_log.read_text())
                assert b"v1 locked 1" in status(), "Turning displays on must not unlock"
                print("Session smoke passed: real swaylock, confirmation, no duplicate, logout blocked, unlock event, idle lock before standby, resume stays locked")
            finally:
                if idle is not None and idle.poll() is None:
                    idle.terminate()
                    idle.wait(timeout=5)
                if river.poll() is None:
                    os.killpg(river.pid, signal.SIGTERM)
                    river.wait(timeout=3)
                log.seek(0)
                print(log.read())


if __name__ == "__main__":
    main()
