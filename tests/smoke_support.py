"""Helpers for isolated session tests."""
from pathlib import Path
import time

ROOT = Path(__file__).resolve().parents[1]


def wait_for(fn):
    deadline = time.monotonic() + 8
    while time.monotonic() < deadline:
        result = fn()
        if result:
            return result
        time.sleep(0.05)
    raise AssertionError("Timed out waiting for WM/session")
