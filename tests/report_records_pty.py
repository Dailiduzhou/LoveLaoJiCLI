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
import subprocess
import sys
import tempfile
import time

bindir = Path(sys.argv[1]).resolve()
with tempfile.TemporaryDirectory(prefix="reports-pty-") as temp:
    root = Path(temp)
    repo = root / "repo"
    repo.mkdir()
    env = dict(os.environ, HOME=temp, XDG_STATE_HOME=str(root / "state"),
               LC_ALL="C", TZ="UTC", GIT_CONFIG_GLOBAL="/dev/null", GIT_CONFIG_NOSYSTEM="1")
    for key in ["GIT_DIR", "GIT_WORK_TREE", "GIT_COMMON_DIR", "GIT_INDEX_FILE", "GIT_CONFIG_COUNT", "GIT_CONFIG_PARAMETERS"]:
        env.pop(key, None)
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
            assert os.waitstatus_to_exitcode(status) == expected, (tool, output, status)
            return output
        finally:
            if status is None:
                os.killpg(pid, signal.SIGKILL)
                os.waitpid(pid, 0)
            os.close(fd)

    def report(tool):
        before = {str(p): p.read_bytes() for p in (root / "state").rglob("*") if p.is_file()}
        r = subprocess.run([bindir / tool], cwd=repo, env=env, capture_output=True, timeout=15)
        assert r.returncode == 0, r
        after = {str(p): p.read_bytes() for p in (root / "state").rglob("*") if p.is_file()}
        assert before == after, "report mutated state"
        return r.stdout

    assert b"completion date): 0" in report("proof")
    assert not (root / "state").exists()
    cmd = ["sh", "-c", "printf ran"]
    wrapper("enough", cmd, 0)
    assert b"already passed" in wrapper("enough", cmd, 0)
    out = report("proof")
    assert b"completion date): 1" in out, out
    assert b"Successful executions (not necessarily tests): 1" in out, out
    fail = ["sh", "-c", "printf failed; exit 7"]
    for _ in range(3):
        wrapper("stuck", fail, 7)
    wrapper("stuck", fail, 125, reply=b"\x04")
    out = report("proof")
    assert b"completion date): 4" in out, out
    assert b"Successful executions (not necessarily tests): 1" in out, out
    out = report("goodnight")
    assert b"not necessarily tests" in out and b"tests passed" not in out, out
    print("cross-tool PTY acceptance passed (real executions, skip, gate, read-only reports)")
