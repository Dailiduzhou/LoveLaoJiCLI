"""A bounded foreground PTY with a session guardian that outlives the tool.

The guardian acts as a tiny shell: separate worker process group, foreground
handoff before exec, and status events over a pipe. It keeps the controlling
session alive until the test has inspected termios/screen cleanup. Never compare
termios after the session leader has exited (ENOTTY on macOS is fixture teardown,
not evidence of broken restoration). No user's terminal/session is touched.
"""
import errno
import fcntl
import os
import pty
import resource
import select
import signal
import struct
import sys
import termios
import time


def set_size(fd, rows, columns):
    fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", rows, columns, 0, 0))


def assert_restored(fd, original):
    actual = termios.tcgetattr(fd)
    # Darwin's tty driver synthesizes PENDIN when ICANON is restored, even
    # after tcsetattr(original). A nonblocking read processes that kernel-only
    # reinput flag. Require an empty queue (do not flush or mask differences),
    # restore descriptor flags, then compare every termios field exactly.
    pending = getattr(termios, "PENDIN", 0)
    expected_pending = list(original)
    expected_pending[3] |= pending
    if sys.platform == "darwin" and actual != original and actual == expected_pending:
        flags = fcntl.fcntl(fd, fcntl.F_GETFL)
        try:
            fcntl.fcntl(fd, fcntl.F_SETFL, flags | os.O_NONBLOCK)
            try:
                queued = os.read(fd, 1)
            except BlockingIOError:
                queued = b""
            assert not queued, ("input leaked after restoration", queued)
        finally:
            fcntl.fcntl(fd, fcntl.F_SETFL, flags)
        actual = termios.tcgetattr(fd)
    assert actual == original, ("termios not restored", original, actual)


class PtySession:
    def __init__(self, argv, env, cwd, *, stdout_path=None, dimensions=(24, 80),
                 configure=None):
        self.master, self.slave = pty.openpty()
        set_size(self.slave, *dimensions)
        if configure:
            configure(self.slave)
        self.original = termios.tcgetattr(self.slave)
        self.events, event_write = os.pipe()
        release_read, self.release = os.pipe()
        self.status = None
        self.output = b""
        self.worker = None
        self.controller = os.fork()
        if self.controller == 0:
            worker = None
            try:
                os.close(self.master)
                os.close(self.events)
                os.close(self.release)
                os.setsid()
                fcntl.ioctl(self.slave, termios.TIOCSCTTY, 0)
                for fd in (0, 1, 2):
                    os.dup2(self.slave, fd)
                resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
                signal.signal(signal.SIGTTOU, signal.SIG_IGN)
                ready_read, ready_write = os.pipe()
                worker = os.fork()
                if worker == 0:
                    os.close(event_write)
                    os.close(release_read)
                    os.close(ready_write)
                    os.setpgid(0, 0)
                    for sig in (signal.SIGTTOU, signal.SIGTTIN, signal.SIGTSTP,
                                signal.SIGINT, signal.SIGQUIT, signal.SIGPIPE):
                        signal.signal(sig, signal.SIG_DFL)
                    os.read(ready_read, 1)
                    os.close(ready_read)
                    if self.slave > 2:
                        os.close(self.slave)
                    os.chdir(cwd)
                    if stdout_path is not None:
                        out = os.open(stdout_path, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
                        os.dup2(out, 1)
                        os.close(out)
                    os.execve(argv[0], argv, env)
                os.close(ready_read)
                os.setpgid(worker, worker)
                os.tcsetpgrp(self.slave, worker)
                os.write(event_write, struct.pack("!i", worker))
                os.write(ready_write, b"x")
                os.close(ready_write)
                while True:
                    _, status = os.waitpid(worker, os.WUNTRACED)
                    os.write(event_write, struct.pack("!i", status))
                    if not os.WIFSTOPPED(status):
                        break
                # Stay alive even after waitpid: the parent still needs a valid
                # terminal to inspect exact restoration and queued input.
                os.tcsetpgrp(self.slave, os.getpgrp())
                os.read(release_read, 1)
                os._exit(0)
            except BaseException:
                import traceback
                traceback.print_exc()
                if worker:
                    try:
                        os.killpg(worker, signal.SIGKILL)
                        os.waitpid(worker, 0)
                    except ProcessLookupError:
                        pass
                os._exit(1)
        os.close(event_write)
        os.close(release_read)
        try:
            self.worker = self._event(5)
        except BaseException:
            self.close()
            raise

    def _event(self, timeout):
        if not select.select([self.events], [], [], timeout)[0]:
            raise AssertionError(("PTY controller event timed out", self.diagnostics()))
        packet = os.read(self.events, 4)
        assert len(packet) == 4, ("PTY controller exited before status", self.diagnostics())
        return struct.unpack("!i", packet)[0]

    def read(self, timeout=.02):
        if select.select([self.master], [], [], timeout)[0]:
            try:
                data = os.read(self.master, 65536)
            except OSError as error:
                if error.errno != errno.EIO:
                    raise
                data = b""
            self.output += data
            return data
        return b""

    def drain(self):
        while select.select([self.master], [], [], 0)[0]:
            if not self.read(0):
                break

    def poll(self):
        if self.status is None and select.select([self.events], [], [], 0)[0]:
            status = self._event(0)
            if not os.WIFSTOPPED(status):
                self.status = status
            return self.worker, status
        return 0, 0

    def diagnostics(self):
        try:
            attrs = termios.tcgetattr(self.slave)
        except termios.error as error:
            attrs = repr(error)
        return {"worker": self.worker, "status": self.status,
                "termios": attrs, "last_output": self.output[-2000:]}

    def close(self):
        if self.worker is not None:
            try:
                os.killpg(self.worker, signal.SIGKILL)
            except ProcessLookupError:
                pass
        try:
            os.write(self.release, b"x")
        except BrokenPipeError:
            pass
        # Disconnect only after assertions, before reaping the guardian. Darwin
        # can otherwise wait in tty close/drain forever after an echoed paste
        # while the test is no longer reading the master (even after SIGKILL).
        os.close(self.master)
        os.close(self.slave)
        deadline = time.monotonic() + 3
        while time.monotonic() < deadline:
            if os.waitpid(self.controller, os.WNOHANG)[0]:
                break
            time.sleep(.01)
        else:
            os.kill(self.controller, signal.SIGKILL)
            os.waitpid(self.controller, 0)
        for fd in (self.events, self.release):
            os.close(fd)

    def __enter__(self):
        return self

    def __exit__(self, *_):
        self.close()
