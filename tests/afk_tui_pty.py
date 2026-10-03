"""Real controlling-terminal lifecycle tests. Every terminal/HOME is disposable."""
import errno
import fcntl
import os
from pathlib import Path
import pty
import re
import resource
import select
import signal
import struct
import sys
import tempfile
import termios
import time

binary = str(Path(sys.argv[1]).resolve())
ENTER = b"\x1b[?1049h"
LEAVE = b"\x1b[?1049l"
SHOW = b"\x1b[?25h"


def has_foreground_color(output):
    return any(re.search(rb"(^|;)(3[0-7]|9[0-7]|38)(;|$)", codes)
               for codes in re.findall(rb"\x1b\[([0-9;]*)m", output))


def size(fd, rows, columns):
    fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", rows, columns, 0, 0))


with tempfile.TemporaryDirectory(prefix="afk-tui-") as temp:
    env = dict(os.environ, HOME=temp, XDG_STATE_HOME=str(Path(temp) / "state"),
               TERM="xterm", LC_ALL="C", NO_COLOR="1", AFK_FAST="1")
    env.pop("AFK_TEST_FAILURE", None)

    def run(action=None, fault=None, expected=0, term="xterm", dimensions=(18, 60),
            duration="100s", chinese=False, suspend_for=None, no_color="1"):
        master, slave = pty.openpty()
        size(slave, *dimensions)
        original = termios.tcgetattr(slave)
        # Verify exact restoration of non-default attributes, not merely ECHO on.
        original[0] &= ~termios.IXON
        termios.tcsetattr(slave, termios.TCSANOW, original)
        original = termios.tcgetattr(slave)
        pid = os.fork()
        if pid == 0:
            os.chdir(temp)
            resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
            os.close(master)
            os.setsid()
            fcntl.ioctl(slave, termios.TIOCSCTTY, 0)
            for fd in (0, 1, 2):
                os.dup2(slave, fd)
            if slave > 2:
                os.close(slave)
            child_env = dict(env, TERM=term)
            if no_color is None:
                child_env.pop("NO_COLOR", None)
            else:
                child_env["NO_COLOR"] = no_color
            if fault:
                child_env["AFK_TEST_FAILURE"] = fault
            if chinese:
                child_env["LC_ALL"] = "zh_CN"
            os.execve(binary, [binary, duration], child_env)
        output = b""
        start = time.monotonic()
        status = None
        acted = False
        was_stopped = False
        resume_at = None
        try:
            while time.monotonic() - start < 8:
                if select.select([master], [], [], .01)[0]:
                    try:
                        output += os.read(master, 65536)
                    except OSError as e:
                        if e.errno != errno.EIO:
                            raise
                elapsed = time.monotonic() - start
                if not acted and ENTER in output and elapsed > .15:
                    attrs = termios.tcgetattr(slave)
                    assert not attrs[3] & (termios.ECHO | termios.ICANON), attrs
                    assert attrs[3] & termios.ISIG
                    if action == "keys":
                        os.write(master, b"q\x1b[Ahello")  # q/Escape are ordinary input, not quits
                    elif action == "ctrl-c":
                        os.write(master, b"\x03")
                    elif action in ("resize", "giant"):
                        size(slave, *((3, 12) if action == "resize" else (65535, 65535)))
                        os.kill(pid, signal.SIGWINCH)
                    elif action == "suspend":
                        os.write(master, b"\x1a")
                    elif isinstance(action, int):
                        os.kill(pid, action)
                    acted = True
                if action == "flood" and ENTER in output and elapsed < .8:
                    os.write(master, b"x" * 100)
                if resume_at is not None and time.monotonic() >= resume_at:
                    assert termios.tcgetattr(slave) == original
                    os.kill(pid, signal.SIGCONT)
                    resume_at = None
                child, child_status = os.waitpid(pid, os.WNOHANG | os.WUNTRACED)
                if child and os.WIFSTOPPED(child_status):
                    assert action == "suspend", (action, child_status)
                    assert termios.tcgetattr(slave) == original, "suspended with modified termios"
                    was_stopped = True
                    resume_at = time.monotonic() + suspend_for
                elif child:
                    status = child_status
                    break
            assert status is not None, ("hung", action, fault, output[-2000:])
            while select.select([master], [], [], 0)[0]:
                try:
                    chunk = os.read(master, 65536)
                except OSError:
                    break
                if not chunk:
                    break
                output += chunk
            assert os.waitstatus_to_exitcode(status) == expected, (action, fault, status, output[-3000:])
            assert termios.tcgetattr(slave) == original, (action, fault, "termios not restored")
            assert b"\x1b[6n" not in output, "cursor query can enable raw mode and consume keys"
            if no_color:
                assert not has_foreground_color(output), "NO_COLOR must suppress scene colors"
            if term != "dumb" and dimensions[0] >= 10 and dimensions[1] >= 24:
                assert ENTER in output and LEAVE in output and SHOW in output, output
                assert output.rfind(LEAVE) > output.rfind(ENTER), output
                assert b"\x1b[0m" + SHOW + LEAVE in output, "cleanup must reset colors"
            else:
                assert ENTER not in output and b"\x1b" not in output, output
            if expected != 0:
                assert b"Break complete." not in output
            if action == "suspend":
                assert was_stopped
            if action == "flood":
                os.set_blocking(slave, False)
                os.write(master, b"\n")
                assert select.select([slave], [], [], .5)[0]
                assert os.read(slave, 8192) == b"\n", "queued TUI input leaked to the shell"
            return output, time.monotonic() - start
        finally:
            if status is None:
                os.kill(pid, signal.SIGKILL)
                os.waitpid(pid, 0)
            os.close(master)
            os.close(slave)

    for no_color in (None, ""):
        out, _ = run(no_color=no_color)
        assert has_foreground_color(out), "scene should be colored by default"
    run(action="ctrl-c", expected=-signal.SIGINT, duration="24h", no_color=None)

    out, elapsed = run(action="keys")
    visible = re.sub(rb"\x1b\[[0-9;?]*[A-Za-z]", b"", out)
    assert b"thegrassnoticed." in visible and b"Break complete." in out, out
    assert .95 <= elapsed < 3, elapsed
    out, elapsed = run(action="flood")
    assert .95 <= elapsed < 3, elapsed
    run(action="resize")
    run(action="giant")
    out, _ = run(chinese=True)
    visible = re.sub(rb"\x1b\[[0-9;?]*[A-Za-z]", b"", out)
    assert "小片草地".encode() in visible and "休息结束。".encode() in out, out
    run(action="ctrl-c", expected=-signal.SIGINT, duration="24h")
    for sig in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP, signal.SIGQUIT):
        run(action=sig, expected=-sig, duration="24h")
    for fault, code in (("enter", 1), ("draw", 1), ("panic", 101)):
        run(fault=fault, expected=code)
    run(term="dumb")
    run(dimensions=(3, 12))
    out, elapsed = run(action="suspend", suspend_for=.25, duration="150s")
    assert out.count(ENTER) >= 2 and 1.45 <= elapsed < 3, (elapsed, out)
    out, elapsed = run(action="suspend", suspend_for=1.1)
    assert out.count(ENTER) == 1 and 1.2 <= elapsed < 2.5, (elapsed, out)

    # A miniature shell in the tty session exercises bg and subsequent fg. The
    # reporting test process cannot tcsetpgrp a terminal in another session.
    master, slave = pty.openpty()
    size(slave, 18, 60)
    original = termios.tcgetattr(slave)
    controller = os.fork()
    if controller == 0:
        os.chdir(temp)
        resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
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
            os.execve(binary, [binary, "24h"], env)
        os.close(ready_read)
        try:
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
            os.kill(worker, signal.SIGTSTP)
            until = time.monotonic() + 3
            while time.monotonic() < until:
                child, status = os.waitpid(worker, os.WNOHANG | os.WUNTRACED)
                if child:
                    assert os.WIFSTOPPED(status), status
                    break
                time.sleep(.01)
            else:
                raise AssertionError("did not suspend")
            assert termios.tcgetattr(slave) == original
            os.tcsetpgrp(slave, os.getpgrp())
            os.kill(worker, signal.SIGCONT)
            time.sleep(.3)
            assert termios.tcgetattr(slave) == original, "background resume reacquired terminal"
            os.tcsetpgrp(slave, worker)
            os.kill(worker, signal.SIGCONT)
            wait_mode(False)
            os.kill(worker, signal.SIGTERM)
            _, status = os.waitpid(worker, 0)
            assert os.waitstatus_to_exitcode(status) == -signal.SIGTERM, status
            assert termios.tcgetattr(slave) == original
        except BaseException:
            os.kill(worker, signal.SIGKILL)
            import traceback
            traceback.print_exc()
            os._exit(1)
        os._exit(0)
    status = None
    output = b""
    try:
        until = time.monotonic() + 10
        while time.monotonic() < until:
            if select.select([master], [], [], .02)[0]:
                output += os.read(master, 65536)
            child, child_status = os.waitpid(controller, os.WNOHANG)
            if child:
                status = child_status
                break
        assert status is not None and os.waitstatus_to_exitcode(status) == 0, output
        assert termios.tcgetattr(slave) == original
    finally:
        if status is None:
            os.kill(controller, signal.SIGKILL)
            os.waitpid(controller, 0)
        os.close(master)
        os.close(slave)
    assert not (Path(temp) / "state").exists()
    print("afk TUI PTY lifecycle passed")
