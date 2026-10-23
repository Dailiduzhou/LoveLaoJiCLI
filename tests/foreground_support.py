"""Foreground prompts with separate stdout, a live session and bounded input/I/O."""
import os
from pathlib import Path
import signal
import tempfile
import termios
import time
from contextlib import contextmanager
from types import SimpleNamespace
from pty_support import PtySession


@contextmanager
def foreground_fixture(binary, extra_env=None):
    binary = str(Path(binary).resolve())
    tool = Path(binary).name
    with tempfile.TemporaryDirectory(prefix="batch-two-pty-") as temp:
        root = Path(temp).resolve()
        env = dict(os.environ, HOME=str(root), XDG_STATE_HOME=str(root / "state"),
                   LC_ALL="C", TERM="xterm")
        env.update(extra_env or {})

        def run(args, replies=(), cancel=None, expected=0, stdout_path=None,
                term="xterm", timeout=10, write_chunk=65536):
            output_path = stdout_path or root / "stdout"
            with PtySession([binary, *args], dict(env, TERM=term), root,
                            stdout_path=output_path) as session:
                fd = session.master
                os.set_blocking(fd, False)
                replies = list(replies)
                pending = b""
                start = time.monotonic()
                status = None
                while time.monotonic() - start < timeout:
                    if cancel and time.monotonic() - start > .2:
                        if cancel == "ctrl-c":
                            pending += b"\x03"
                        elif cancel == "keys":
                            pending += b"hello\n"
                        else:
                            os.kill(session.worker, cancel)
                        cancel = None
                    session.read(.01)
                    if not pending and replies and replies[0][0] in session.output:
                        _, pending = replies.pop(0)
                        assert all(t not in session.output for t, _ in replies), session.output
                    if pending:
                        try:
                            written = os.write(fd, pending[:write_chunk])
                            pending = pending[written:]
                        except BlockingIOError:
                            pass
                    child, child_status = session.poll()
                    if child:
                        assert not os.WIFSTOPPED(child_status), session.diagnostics()
                        status = child_status
                        break
                assert status is not None, ("product timeout", tool, session.diagnostics())
                session.drain()
                # The guardian is still alive; ENOTTY here is a controller error,
                # not a reason to suppress the full termios assertion.
                assert termios.tcgetattr(session.slave) == session.original, (tool, args)
                code = os.waitstatus_to_exitcode(status)
                assert code == expected, (args, code, session.output[-3000:])
                assert not replies and not pending, ("missing prompts/input", replies, pending, session.output[-3000:])
                saved = output_path.read_bytes() if stdout_path is None else b""
                return saved, session.output, time.monotonic() - start

        yield SimpleNamespace(root=root, env=env, run=run)
