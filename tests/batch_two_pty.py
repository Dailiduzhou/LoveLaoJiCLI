"""Foreground terminal checks with separate stdout and bounded child lifetimes."""
import errno
import os
from pathlib import Path
import pty
import select
import signal
import subprocess
import sys
import tempfile
import termios
import time

binary, tool = sys.argv[1:]
with tempfile.TemporaryDirectory(prefix="batch-two-pty-") as temp:
    root = Path(temp)
    env = dict(os.environ, HOME=temp, XDG_STATE_HOME=str(root / "state"),
               LC_ALL="C", TERM="xterm", AFK_FAST="1")

    def run(args, replies=(), cancel=None, expected=0, stdout_path=None, term="xterm"):
        output_path = stdout_path or root / "stdout"
        pid, fd = pty.fork()
        if pid == 0:
            os.chdir(root)
            out = os.open(output_path, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
            os.dup2(out, 1)
            os.close(out)
            os.execve(binary, [binary, *args], dict(env, TERM=term))
        original = termios.tcgetattr(fd)
        output = b""
        replies = list(replies)
        start = time.monotonic()
        status = None
        try:
            while time.monotonic() - start < 10:
                if cancel and time.monotonic() - start > .2:
                    if cancel == "ctrl-c":
                        os.write(fd, b"\x03")
                    elif cancel == "keys":
                        os.write(fd, b"hello\n")
                    else:
                        os.kill(pid, cancel)
                    cancel = None
                if select.select([fd], [], [], .02)[0]:
                    try:
                        chunk = os.read(fd, 65536)
                    except OSError as e:
                        if e.errno != errno.EIO:
                            raise
                        chunk = b""
                    output += chunk
                if replies and replies[0][0] in output:
                    trigger, reply = replies.pop(0)
                    # Future questions may only appear after the current answer.
                    assert all(t not in output for t, _ in replies), output
                    os.write(fd, reply)
                child, child_status = os.waitpid(pid, os.WNOHANG)
                if child:
                    status = child_status
                    break
            assert status is not None, ("hung", tool, output)
            # No invocation changes terminal attributes, including error/signal paths.
            assert termios.tcgetattr(fd) == original, (tool, args)
            code = os.waitstatus_to_exitcode(status)
            assert code == expected, (args, code, output)
            assert not replies, ("missing prompts", replies, output)
            saved = output_path.read_bytes() if stdout_path is None else b""
            return saved, output, time.monotonic() - start
        finally:
            if status is None:
                os.kill(pid, signal.SIGKILL)
                os.waitpid(pid, 0)
            os.close(fd)

    if tool == "duck":
        questions = [(b"Expected?", b"success\n"), (b"Actual?", b"failure\n"),
                     (b"Last change?", b"timeout\n"), (b"Next experiment?", b"increase it\n")]
        expected = b"Expected: success\nActual: failure\nLast change: timeout\nNext experiment: increase it\nquack.\n"
        out, prompts, _ = run([], questions)
        assert out == expected, out
        for question, _ in questions:
            assert question in prompts
            assert question not in out
        out, prompts, _ = run(["--expected", "success", "--last-change", "timeout"],
                              [questions[1], questions[3]])
        assert out == expected
        assert b"Expected?" not in prompts and b"Last change?" not in prompts
        for answer in [b"\x04", b"   \n", b"x" * 2001 + b"\n"]:
            out, _, _ = run([], [(b"Expected?", answer)], expected=125)
            assert out == b""
        out, _, _ = run([], cancel="ctrl-c", expected=-signal.SIGINT)
        assert out == b""
        out, prompts, _ = run(["--expected", "success", "--actual", "failure",
                               "--last-change", "timeout", "--experiment", "increase it"])
        assert out == expected and prompts == b""
        assert not (root / "state").exists()
    elif tool == "afk":
        # stdout is redirected, so afk must use plain text even when stdin and
        # stderr are foreground terminals. Real TUI coverage is in afk_tui_pty.py.
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
