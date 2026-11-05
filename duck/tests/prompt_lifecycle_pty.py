"""Shared prompt terminal lifecycle, exercised through duck (no state).

Covers: cbreak with ISIG while asking, Ctrl+C/SIGTERM restore before dying,
Ctrl+Z restore before stopping, fg re-entry, bg never touching the terminal,
rejected pastes never leaking to the next reader, and no cursor queries.
"""
from pathlib import Path
import os
import select
import signal
import sys
import termios
import time
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "tests"))
from pty_support import PtySession, assert_restored

binary = str(Path(sys.argv[1]).resolve())
REST = ["--actual", "a", "--last-change", "b", "--experiment", "c"]


def env(root):
    return dict(os.environ, HOME=str(root), XDG_STATE_HOME=str(root / "state"),
                LC_ALL="C", TERM="xterm")


def wait_for(session, predicate, what, timeout=3):
    until = time.monotonic() + timeout
    while time.monotonic() < until:
        session.read(.01)
        if predicate():
            return
    raise AssertionError((what, session.diagnostics()))


def cbreak(session):
    attrs = termios.tcgetattr(session.slave)
    return not attrs[3] & (termios.ICANON | termios.ECHO) and attrs[3] & termios.ISIG


def finish(session, timeout=3):
    until = time.monotonic() + timeout
    while time.monotonic() < until:
        session.read(.01)
        child, status = session.poll()
        if child and not os.WIFSTOPPED(status):
            session.drain()
            return os.waitstatus_to_exitcode(status)
    raise AssertionError(("product timeout", session.diagnostics()))


def start(root, args=REST):
    session = PtySession([binary, *args], env(root), root, stdout_path=root / "stdout")
    os.set_blocking(session.master, False)
    wait_for(session, lambda: b"Expected?" in session.output and cbreak(session),
             "prompt never entered cbreak with ISIG")
    return session


def cancel_with_key(root, key, expected):
    with start(root) as session:
        os.write(session.master, b"partial " + key)
        code = finish(session)
        assert code == expected, (key, code, session.output[-500:])
        assert_restored(session.slave, session.original)
        assert (root / "stdout").read_bytes() == b""


def cancel_with_signal(root, sig, expected):
    with start(root) as session:
        os.write(session.master, b"partial answer")
        wait_for(session, lambda: b"partial answer" in session.output, "echo missing")
        os.kill(session.worker, sig)
        code = finish(session)
        assert code == expected, (sig, code, session.output[-500:])
        assert_restored(session.slave, session.original)
        assert (root / "stdout").read_bytes() == b""


def suspend_and_resume(root):
    with start(root) as session:
        os.write(session.master, "半个".encode())
        wait_for(session, lambda: "半个".encode() in session.output, "echo missing")
        os.write(session.master, b"\x1a")
        until = time.monotonic() + 3
        while time.monotonic() < until:
            session.read(.01)
            child, status = session.poll()
            if child:
                assert os.WIFSTOPPED(status), status
                break
        else:
            raise AssertionError(("did not stop", session.diagnostics()))
        # Stopped with the original settings live; nothing queued is lost.
        assert_restored(session.slave, session.original)
        os.kill(session.worker, signal.SIGCONT)  # fg: still the foreground group
        wait_for(session, lambda: cbreak(session), "fg did not re-enter cbreak")
        os.write(session.master, "答案\n".encode())
        assert finish(session) == 0
        assert_restored(session.slave, session.original)
        assert (root / "stdout").read_bytes().startswith("Expected: 半个答案\n".encode())
        # fg re-shows the question and the partial answer.
        assert session.output.count(b"Expected?") == 2, session.output


def rejected_paste_never_leaks(root):
    # An invalid line plus the rest of the paste: the remainder is discarded,
    # never left for the shell or the next reader on this terminal.
    with start(root) as session:
        os.write(session.master, b"bad\x01line\nrm -rf leftover\n")
        assert finish(session) == 125
        assert_restored(session.slave, session.original)
        os.set_blocking(session.slave, False)
        try:
            leaked = os.read(session.slave, 4096)
        except BlockingIOError:
            leaked = b""
        assert leaked == b"", leaked
        assert (root / "stdout").read_bytes() == b""
        assert b"\x1b[6n" not in session.output


def background_never_reads_or_draws(root):
    # A miniature shell inside the PTY session moves duck to the background and
    # back. The reporting process cannot tcsetpgrp a terminal in another session.
    import fcntl
    import pty
    import resource
    master, slave = pty.openpty()
    original = termios.tcgetattr(slave)
    controller = os.fork()
    if controller == 0:
        try:
            os.close(master)
            os.setsid()
            fcntl.ioctl(slave, termios.TIOCSCTTY, 0)
            for fd in (0, 1, 2):
                os.dup2(slave, fd)
            resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
            signal.signal(signal.SIGTTOU, signal.SIG_IGN)
            ready_read, ready_write = os.pipe()
            worker = os.fork()
            if worker == 0:
                os.close(ready_write)
                os.setpgid(0, 0)
                for sig in (signal.SIGTTOU, signal.SIGTTIN, signal.SIGTSTP, signal.SIGINT):
                    signal.signal(sig, signal.SIG_DFL)
                os.read(ready_read, 1)
                out = os.open(root / "stdout", os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
                os.dup2(out, 1)
                os.execve(binary, [binary, *REST], env(root))
            os.close(ready_read)
            os.setpgid(worker, worker)
            os.tcsetpgrp(slave, worker)
            os.write(ready_write, b"x")
            os.close(ready_write)

            def wait_mode(canonical):
                until = time.monotonic() + 3
                while time.monotonic() < until:
                    if bool(termios.tcgetattr(slave)[3] & termios.ICANON) == canonical:
                        return
                    time.sleep(.01)
                raise AssertionError("terminal mode transition timed out")
            wait_mode(False)
            # bg: the shell takes the terminal and, like a real shell, installs
            # its own settings. Duck must never write its stale snapshot or
            # read/draw while it is not the foreground group.
            os.tcsetpgrp(slave, os.getpgrp())
            termios.tcsetattr(slave, termios.TCSANOW, original)
            time.sleep(.4)
            assert_restored(slave, original)
            assert os.waitpid(worker, os.WNOHANG) == (0, 0), "prompt exited in bg"
            # fg: give it back; it re-enters cbreak, redraws, and accepts the
            # answer the reporting side sends after seeing the second prompt.
            wait_mode(True)  # bg shell settings still live here
            os.tcsetpgrp(slave, worker)
            _, status = os.waitpid(worker, 0)
            os.tcsetpgrp(slave, os.getpgrp())
            assert_restored(slave, original)
            os._exit(0 if os.waitstatus_to_exitcode(status) == 0 else 3)
        except BaseException:
            import traceback
            traceback.print_exc()
            os._exit(1)
    output = b""
    status = None
    answered = False
    try:
        until = time.monotonic() + 10
        while time.monotonic() < until:
            if select.select([master], [], [], .02)[0]:
                try:
                    output += os.read(master, 65536)
                except OSError:
                    pass
            # Answer only after fg has redrawn the question a second time.
            if not answered and output.count(b"Expected?") >= 2:
                os.write(master, b"after fg\n")
                answered = True
            child, child_status = os.waitpid(controller, os.WNOHANG)
            if child:
                status = child_status
                break
        assert status is not None, ("bg/fg timed out", output[-1000:])
        assert os.waitstatus_to_exitcode(status) == 0, (status, output[-2000:])
        assert (root / "stdout").read_bytes().startswith(b"Expected: after fg\n")
    finally:
        if status is None:
            os.kill(controller, signal.SIGKILL)
            os.waitpid(controller, 0)
        os.close(master)
        os.close(slave)


def main():
    import tempfile
    for scenario in (
        lambda r: cancel_with_key(r, b"\x03", -signal.SIGINT),
        lambda r: cancel_with_signal(r, signal.SIGTERM, -signal.SIGTERM),
        lambda r: cancel_with_signal(r, signal.SIGHUP, -signal.SIGHUP),
        suspend_and_resume,
        rejected_paste_never_leaks,
        background_never_reads_or_draws,
    ):
        with tempfile.TemporaryDirectory(prefix="duck-prompt-") as temp:
            scenario(Path(temp).resolve())
    print("duck prompt lifecycle PTY passed")


if __name__ == "__main__":
    main()
