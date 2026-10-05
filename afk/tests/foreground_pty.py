"""Tool-owned foreground prompt/plain-output scenarios."""
from pathlib import Path
import signal
import subprocess
import sys
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "tests"))
from foreground_support import foreground_fixture

binary = sys.argv[1]
with foreground_fixture(binary, {"AFK_FAST": "1"}) as f:
    root, env, run = f.root, f.env, f.run
    # stdout is redirected, so afk must use plain text even when stdin and
    # stderr are foreground terminals. Real TUI coverage is in afk/tests/afk_tui_pty.py.
    for term in ["xterm", "dumb"]:
        out, _, elapsed = run(["100s"], cancel="keys", term=term)
        assert out == b"Break complete.\n"
        assert .95 <= elapsed < 5, elapsed
    for cancel in ["ctrl-c", signal.SIGTERM, signal.SIGHUP]:
        expected = -signal.SIGINT if cancel == "ctrl-c" else -cancel
        out, _, _ = run(["24h"], cancel=cancel, expected=expected)
        assert out == b""
    run(["0s"], expected=2)
    if Path("/dev/full").exists():
        run(["1s"], expected=1, stdout_path=Path("/dev/full"))
    # A pipe that has not reached EOF must not become an input dependency.
    child = subprocess.Popen([binary, "1s"], stdin=subprocess.PIPE,
                             stdout=subprocess.PIPE, stderr=subprocess.PIPE, env=env)
    try:
        assert child.wait(timeout=5) == 0
        assert child.stdout.read() == b"Break complete.\n"
    finally:
        if child.poll() is None:
            child.kill()
            child.wait()
        child.stdin.close()
        child.stdout.close()
        child.stderr.close()
    assert not (root / "state").exists()
