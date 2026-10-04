"""Do not restore stale settings or write cleanup escapes into a new owner's TTY."""
import errno
import fcntl
import os
from pathlib import Path
import pty
import select
import signal
import struct
import sys
import tempfile
import termios
import time

binary = str(Path(sys.argv[1]).resolve())
marker = b"NEW_FOREGROUND_OWNER\r\n"


def wait_child(pid, flags=0):
    deadline = time.monotonic() + 3
    while time.monotonic() < deadline:
        child, status = os.waitpid(pid, os.WNOHANG | flags)
        if child:
            return status
        time.sleep(.01)
    raise AssertionError("worker did not change state")


with tempfile.TemporaryDirectory(prefix="afk-owner-") as home:
    # Exercise ordinary and termination/Drop disposal, plus deterministic loss
    # between the event loop's foreground check and its input read.
    for read_race, immediate_signal in ((False, False), (False, True),
                                        (True, False), (True, True)):
        master, slave = pty.openpty()
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 18, 60, 0, 0))
        original = termios.tcgetattr(slave)
        controller = os.fork()
        if controller == 0:
            os.close(master)
            os.setsid()
            fcntl.ioctl(slave, termios.TIOCSCTTY, 0)
            for fd in (0, 1, 2):
                os.dup2(slave, fd)
            signal.signal(signal.SIGTTOU, signal.SIG_IGN)
            ready_read, ready_write = os.pipe()
            worker = os.fork()
            if worker == 0:
                os.close(ready_write)
                os.setpgid(0, 0)
                os.read(ready_read, 1)
                os.close(ready_read)
                env = dict(os.environ, HOME=home, TERM="xterm", LC_ALL="C", NO_COLOR="1")
                env.pop("AFK_FAST", None)
                env.pop("AFK_TEST_FAILURE", None)
                if read_race:
                    env["AFK_TEST_FAILURE"] = "read-stop"
                os.execve(binary, [binary, "30s"], env)
            os.close(ready_read)
            reaped = False
            try:
                os.setpgid(worker, worker)
                os.tcsetpgrp(slave, worker)
                os.write(ready_write, b"x")
                os.close(ready_write)
                deadline = time.monotonic() + 3
                while termios.tcgetattr(slave)[3] & termios.ICANON:
                    assert time.monotonic() < deadline, "TUI did not enter"
                    time.sleep(.01)
                if not read_race:
                    # Allow a frame to hide the cursor, covering Ratatui's Drop.
                    time.sleep(.2)
                    os.kill(worker, signal.SIGSTOP)
                # read-stop freezes inside afk after its foreground check but
                # before read(2), making the background EIO race reproducible.
                # Restoration during SIGSTOP is not the behavior under test.
                stopped = wait_child(worker, os.WUNTRACED)
                assert os.WIFSTOPPED(stopped), stopped
                assert os.WSTOPSIG(stopped) == signal.SIGSTOP, stopped
                os.tcsetpgrp(slave, os.getpgrp())
                replacement = termios.tcgetattr(slave)
                replacement[0] ^= termios.IXON
                replacement[3] &= ~(termios.ECHO | termios.ICANON)
                assert replacement != original
                termios.tcsetattr(slave, termios.TCSANOW, replacement)
                os.write(1, marker)
                if immediate_signal:
                    os.kill(worker, signal.SIGTERM)
                os.kill(worker, signal.SIGCONT)
                if not immediate_signal:
                    time.sleep(.3)  # afk must discard its live session in background
                    assert termios.tcgetattr(slave) == replacement, "stale termios restored"
                    os.kill(worker, signal.SIGTERM)
                status = wait_child(worker)
                reaped = True
                assert os.waitstatus_to_exitcode(status) == -signal.SIGTERM, (
                    read_race, immediate_signal, os.waitstatus_to_exitcode(status))
                assert termios.tcgetattr(slave) == replacement, "Drop restored stale termios"
            except BaseException:
                import traceback
                traceback.print_exc()
                exit_code = 1
            else:
                exit_code = 0
            finally:
                if not reaped:
                    os.kill(worker, signal.SIGKILL)
                    os.waitpid(worker, 0)
            os._exit(exit_code)
        output = b""
        status = None
        try:
            deadline = time.monotonic() + 10
            while time.monotonic() < deadline:
                if select.select([master], [], [], .02)[0]:
                    try:
                        output += os.read(master, 65536)
                    except OSError as error:
                        if error.errno != errno.EIO:
                            raise
                child, child_status = os.waitpid(controller, os.WNOHANG)
                if child:
                    status = child_status
                    break
            while select.select([master], [], [], 0)[0]:
                output += os.read(master, 65536)
            assert status is not None and os.waitstatus_to_exitcode(status) == 0, output
            # ONLCR may map the marker's newline to CRLF again.
            normalized = output.replace(b"\r", b"")
            assert b"\x1b[?1049h" in normalized, output
            assert normalized.split(marker.replace(b"\r", b""), 1)[1] == b"", output
        finally:
            if status is None:
                os.kill(controller, signal.SIGKILL)
                os.waitpid(controller, 0)
            os.close(master)
            os.close(slave)
print("afk foreground ownership transfer passed")
