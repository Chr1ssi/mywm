#!/usr/bin/env python3
"""Actual Wayland and Xwayland fullscreen with a top bar on isolated River."""
import os
from pathlib import Path
import shlex
import signal
import subprocess
import tempfile
import time
from smoke_support import ROOT, wait_for


def main():
    with tempfile.TemporaryDirectory(prefix='mywm-fullscreen-') as directory:
        base = Path(directory)
        runtime = base/'runtime'
        runtime.mkdir(mode=0o700)
        app = base/'app'
        flags = subprocess.check_output(['pkg-config', '--cflags', '--libs', 'gtk+-3.0'], text=True)
        subprocess.run(['cc', str(ROOT/'tests/fixtures/fullscreen.c'), '-o', str(app), *shlex.split(flags)], check=True)
        config = base/'config.toml'
        config.write_text('[appearance]\ngaps_outer = 8\nborder_width = 2\n')
        display = base/'display'
        init = base/'init'
        init.write_text('#!/bin/sh\nprintf "%s" "$DISPLAY" > '+shlex.quote(str(display))+'\nexec '+shlex.quote(str(ROOT/'target/debug/mywm'))+'\n')
        init.chmod(0o755)
        env = dict(os.environ, XDG_RUNTIME_DIR=str(runtime), MYWM_CONFIG=str(config), MYWM_SOCKET=str(runtime/'wm.sock'),
                   WLR_BACKENDS='headless', WLR_HEADLESS_OUTPUTS='1', WLR_RENDERER='pixman',
                   QT_QPA_PLATFORM='wayland', QT_QUICK_BACKEND='software')
        for key in ['WAYLAND_DISPLAY', 'WAYLAND_SOCKET', 'DISPLAY']:
            env.pop(key, None)
        children = []
        with (base/'log').open('w+') as log:
            river = subprocess.Popen(['river', '-c', str(init)], env=env, stdout=log, stderr=log, start_new_session=True)
            try:
                wait_for(lambda: display.exists() and Path(env['MYWM_SOCKET']).exists())
                env['WAYLAND_DISPLAY'] = next(p.name for p in runtime.glob('wayland-*') if p.is_socket())
                env['DISPLAY'] = display.read_text()
                bar = subprocess.Popen([str(ROOT/'target/debug/mywm'), '--bar'], env=env, stdout=log, stderr=log)
                children.append(bar)
                for backend in ['wayland', 'x11']:
                    control, status = base/'control', base/'status'
                    control.write_text('0'); status.unlink(missing_ok=True)
                    client = subprocess.Popen([str(app), str(control), str(status)], env=dict(env, GDK_BACKEND=backend), stdout=log, stderr=log)
                    children.append(client)
                    def size():
                        return status.read_text().strip() if status.exists() else ''
                    # 1280x720, less the 36px bar and 10px margin/border per edge.
                    wait_for(lambda: size() == '1260 664 0')
                    control.write_text('1')
                    wait_for(lambda: size() == '1280 720 1')
                    time.sleep(0.25)
                    shot = base/'shot.ppm'
                    subprocess.run(['grim', '-t', 'ppm', str(shot)], env=env, check=True)
                    with shot.open('rb') as f:
                        assert f.readline() == b'P6\n'
                        assert f.readline().strip() == b'1280 720'
                        assert f.readline().strip() == b'255'
                        pixels = f.read()
                    # The former bar and gap areas now contain the fullscreen client.
                    for x,y in [(10,10),(640,360),(1270,710)]:
                        assert pixels[(y*1280+x)*3:(y*1280+x)*3+3] == b'\xff\0\0', (backend,x,y,list(pixels[(y*1280+x)*3:(y*1280+x)*3+3]))
                    control.write_text('0')
                    wait_for(lambda: size() == '1260 664 0')
                    client.terminate(); client.wait(timeout=5)
                print('Fullscreen smoke passed: native Wayland and Xwayland, full output pixels over bar/gaps, normal geometry restored')
            except Exception:
                log.flush(); log.seek(0); print(log.read())
                raise
            finally:
                for child in children:
                    if child.poll() is None:
                        child.terminate(); child.wait(timeout=5)
                if river.poll() is None:
                    os.killpg(river.pid, signal.SIGTERM)
                river.wait(timeout=5)


if __name__ == '__main__':
    main()
