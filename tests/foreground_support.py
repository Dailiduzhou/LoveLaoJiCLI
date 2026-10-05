"""Foreground terminal checks with separate stdout and bounded child lifetimes."""
import errno
import os
from pathlib import Path
import pty
import select
import signal
import tempfile
import termios
import time

from contextlib import contextmanager
from types import SimpleNamespace

@contextmanager
def foreground_fixture(binary, extra_env=None):
    tool = Path(binary).name
    with tempfile.TemporaryDirectory(prefix="batch-two-pty-") as temp:
        root = Path(temp)
        env = dict(os.environ, HOME=temp, XDG_STATE_HOME=str(root / "state"),
                   LC_ALL="C", TERM="xterm")
        env.update(extra_env or {})

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

        yield SimpleNamespace(root=root, env=env, run=run)
