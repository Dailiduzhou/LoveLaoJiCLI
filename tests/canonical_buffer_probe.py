"""Measure kernel canonical buffering, independently of any product reader.

Usage: python3 tests/canonical_buffer_probe.py
Generated ASCII/CJK only, isolated PTY, bounded lifetime. After a controlled
canonical paste the worker briefly switches mode to inspect the queued bytes,
then restores the original settings. This is diagnostic evidence, not a safe
input implementation or a passing long-input product regression.
"""
import json
import os
from pathlib import Path
import sys
import signal
import tempfile
import termios
import time
from pty_support import PtySession, assert_restored


def worker():
    saved = termios.tcgetattr(0)
    signal.pthread_sigmask(signal.SIG_BLOCK, {signal.SIGUSR1})
    os.write(2, b"PROBE_READY\n")
    # Inspect only after the parent completed the write(2), without a sleep race.
    signal.sigwait({signal.SIGUSR1})
    mode = termios.tcgetattr(0)
    mode[3] &= ~(termios.ICANON | termios.ECHO)
    mode[6][termios.VMIN] = 0
    mode[6][termios.VTIME] = 0
    try:
        termios.tcsetattr(0, termios.TCSANOW, mode)
        data = os.read(0, 8192)
        try:
            data.decode("utf-8")
            valid = True
        except UnicodeDecodeError:
            valid = False
        print(json.dumps({"received_bytes": len(data), "newline_received": b"\n" in data,
                          "valid_utf8": valid}))
    finally:
        termios.tcsetattr(0, termios.TCSANOW, saved)


def probe():
    with tempfile.TemporaryDirectory(prefix="canonical-probe-") as temp:
        root = Path(temp).resolve()
        for name, text in [("ascii", "x" * 600), ("cjk", "中" * 600)]:
            with PtySession([sys.executable, str(Path(__file__).resolve()), "--worker"],
                            dict(os.environ), root, stdout_path=root / "result") as session:
                deadline = time.monotonic() + 3
                while b"PROBE_READY" not in session.output and time.monotonic() < deadline:
                    session.read(.01)
                assert b"PROBE_READY" in session.output, session.diagnostics()
                payload = text.encode() + b"\n"
                assert os.write(session.master, payload) == len(payload)
                os.kill(session.worker, signal.SIGUSR1)
                while not session.poll()[0] and time.monotonic() < deadline:
                    session.read(.01)
                assert session.status == 0, session.diagnostics()
                assert_restored(session.slave, session.original)
                result = json.loads((root / "result").read_text())
                print(json.dumps(dict(case=name, sent_bytes=len(payload),
                                      pc_max_canon=os.fpathconf(session.slave, "PC_MAX_CANON"),
                                      **result)))


if __name__ == "__main__":
    worker() if "--worker" in sys.argv else probe()
