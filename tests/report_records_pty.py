"""Cross-tool acceptance: run after cargo build --locked --workspace --release.
Usage: python3 tests/report_records_pty.py target/release
Uses only isolated HOME/state/repositories and real foreground PTYs.
"""
import errno
import os
from pathlib import Path
import pty
import select
import signal
import stat
import subprocess
import sys
import tempfile
import time

bindir = Path(sys.argv[1]).resolve()
with tempfile.TemporaryDirectory(prefix="reports-pty-") as temp:
    root = Path(temp)
    repo = root / "repo"
    repo.mkdir()
    env = {k: v for k, v in os.environ.items() if not k.startswith("GIT_")}
    env.update(HOME=temp, XDG_STATE_HOME=str(root / "state"), XDG_CONFIG_HOME=str(root / "config"),
               PATH="/usr/bin:/bin", LC_ALL="C", TZ="UTC",
               GIT_CONFIG_GLOBAL="/dev/null", GIT_CONFIG_NOSYSTEM="1")
    def git(*args):
        subprocess.run(["git", *args], cwd=repo, env=env, check=True, capture_output=True, timeout=15)
    git("init", "-q")
    git("config", "user.name", "Test")
    git("config", "user.email", "test@localhost")
    (repo / "code").write_text("one\n")
    git("add", ".")
    git("commit", "-qm", "initial")

    def wrapper(tool, args, expected, reply=None):
        pid, fd = pty.fork()
        if pid == 0:
            os.chdir(repo)
            os.execve(bindir / tool, [tool, *args], env)
        output = b""
        status = None
        deadline = time.monotonic() + 15
        try:
            while time.monotonic() < deadline:
                if select.select([fd], [], [], .02)[0]:
                    try:
                        output += os.read(fd, 65536)
                    except OSError as e:
                        if e.errno != errno.EIO:
                            raise
                if reply is not None and b"What are you changing?" in output:
                    os.write(fd, reply)
                    reply = None
                child, child_status = os.waitpid(pid, os.WNOHANG)
                if child:
                    status = child_status
                    break
            assert status is not None, ("hung", tool, output)
            while select.select([fd], [], [], 0)[0]:
                try:
                    chunk = os.read(fd, 65536)
                except OSError as error:
                    if error.errno != errno.EIO:
                        raise
                    break
                if not chunk:
                    break
                output += chunk
            assert reply is None, ("hypothesis prompt never observed", tool, output)
            assert os.waitstatus_to_exitcode(status) == expected, (tool, output, status)
            return output
        finally:
            if status is None:
                os.killpg(pid, signal.SIGKILL)
                os.waitpid(pid, 0)
            os.close(fd)

    def snapshot():
        state = root / "state"
        if not state.exists():
            return None
        result = {}
        for path in [state, *state.rglob("*")]:
            mode = path.lstat().st_mode
            if stat.S_ISLNK(mode):
                content = os.readlink(path)
            elif stat.S_ISREG(mode):
                content = path.read_bytes()
            else:
                content = None
            result[str(path.relative_to(state))] = (mode, content)
        return result

    def report(tool):
        before = snapshot()
        r = subprocess.run([bindir / tool], cwd=repo, env=env, capture_output=True, timeout=15)
        assert r.returncode == 0, r
        assert before == snapshot(), "report mutated state"
        return r.stdout

    completed = b"Completed executions (current workspace, completion date): "
    successful = b"Successful executions (not necessarily tests): "
    assert completed + b"0" in report("proof").splitlines()
    assert not (root / "state").exists()
    cmd = ["sh", "-c", "printf ran"]
    wrapper("enough", cmd, 0)
    assert b"already passed" in wrapper("enough", cmd, 0)
    out = report("proof")
    assert completed + b"1" in out.splitlines(), out
    assert successful + b"1" in out.splitlines(), out
    fail = ["sh", "-c", "printf failed; exit 7"]
    for _ in range(3):
        wrapper("stuck", fail, 7)
    wrapper("stuck", fail, 125, reply=b"\x04")
    out = report("proof")
    assert completed + b"4" in out.splitlines(), out
    assert successful + b"1" in out.splitlines(), out
    out = report("goodnight")
    assert b"not necessarily tests" in out and b"tests passed" not in out, out
    print("cross-tool PTY acceptance passed (real executions, skip, gate, read-only reports)")
