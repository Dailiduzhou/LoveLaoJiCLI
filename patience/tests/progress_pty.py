"""Observe actual animated stderr; a pipe keeps the real child alive until release."""
import errno
import os
from pathlib import Path
import pty
import re
import select
import signal
import subprocess
import sys
import tempfile
import time

binary = str(Path(sys.argv[1]).resolve())

with tempfile.TemporaryDirectory(prefix="patience-progress-") as temp:
    env = dict(os.environ, HOME=temp, LC_ALL="C", TERM="xterm", NO_COLOR="1",
               PATH="/usr/bin:/bin", PATIENCE_FAST="1", PATIENCE_SEED="42")
    env.pop("PATIENCE_COLOR", None)
    env.pop("PATIENCE_ANGLE", None)

    def run(exit_code, nested=0):
        master, slave = pty.openpty()
        marker = Path(temp) / f"child-{exit_code}-{nested}"
        args = [binary, *(["patience"] * nested), "sh", "-c",
                'printf "started\\n" >> "$1"; printf "CHILD_OUT\\n"; read reply; exit "$2"',
                "child", str(marker), str(exit_code)]
        child = subprocess.Popen(args, env=env, cwd=temp, stdin=subprocess.PIPE,
                                 stdout=subprocess.PIPE, stderr=slave, start_new_session=True)
        os.close(slave)
        output = b""
        bars = nested + 1

        def read_for(seconds):
            nonlocal output
            deadline = time.monotonic() + seconds
            while time.monotonic() < deadline:
                if not select.select([master], [], [], min(.01, max(0, deadline - time.monotonic())))[0]:
                    continue
                try:
                    chunk = os.read(master, 65536)
                except OSError as error:
                    if error.errno == errno.EIO:
                        return
                    raise
                if not chunk:
                    return
                output += chunk

        def progress():
            return [float(value) for value in re.findall(rb"([0-9]+\.[0-9])%", output)]

        try:
            deadline = time.monotonic() + 10
            while time.monotonic() < deadline:
                read_for(.02)
                values = progress()
                if len(values) < bars or not all(97 <= v < 100 for v in values[-bars:]):
                    assert child.poll() is None, (child.returncode, output)
                    continue
                hold = values[-bars:]
                count = len(values)
                read_for(.08)
                new = progress()[count:]
                if len(new) >= bars * 2 and all(v == hold[i % bars] for i, v in enumerate(new)):
                    break
            else:
                raise AssertionError(("no final hold observed", output[-3000:]))
            assert child.poll() is None, "timer/show must not finish before the real child"
            assert max(progress()) < 100, "bar filled before success was known"
            assert not select.select([child.stdout], [], [], 0)[0], "child output replayed before the show ended"
            assert marker.read_text() == "started\n", "nested bars must share exactly one real child"
            if nested:
                assert f"\x1b[{nested}A".encode() in output, "stacked bars were not redrawn"
            child.stdin.write(b"go\n")
            child.stdin.close()
            deadline = time.monotonic() + 10
            while child.poll() is None and time.monotonic() < deadline:
                read_for(.02)
            assert child.poll() == exit_code, (child.poll(), output[-3000:])
            read_for(.05)
            stdout = child.stdout.read()
            assert stdout.startswith(b"CHILD_OUT\n"), stdout
            assert f"exit {exit_code}".encode() in stdout, stdout
            if exit_code == 0:
                assert progress().count(100.0) >= bars, "every successful bar must fill"
                assert "✓ Done!".encode() in stdout, stdout
            else:
                assert max(progress()) < 100, "failed child must never fill the bar"
                assert "✗ It failed.".encode() in stdout, stdout
        finally:
            if child.poll() is None:
                os.killpg(child.pid, signal.SIGKILL)
                child.wait()
            if not child.stdin.closed:
                child.stdin.close()
            child.stdout.close()
            os.close(master)

    run(0)
    run(3)
    run(0, nested=2)
print("patience animated hold/fill/nesting passed")
