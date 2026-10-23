"""Cross-tool acceptance: run after cargo build --locked --workspace --release.
Usage: python3 tests/report_records_pty.py target/release
Uses only isolated HOME/state/repositories and real foreground PTYs.
"""
import os
from pathlib import Path
import stat
import subprocess
import sys
import tempfile
import time
from pty_support import PtySession

bindir = Path(sys.argv[1]).resolve()
with tempfile.TemporaryDirectory(prefix="reports-pty-") as temp:
    root = Path(temp).resolve()
    repo = root / "repo"
    repo.mkdir()
    env = {k: v for k, v in os.environ.items() if not k.startswith("GIT_")}
    env.update(HOME=str(root), XDG_STATE_HOME=str(root / "state"), XDG_CONFIG_HOME=str(root / "config"),
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
        with PtySession([str(bindir / tool), *args], env, repo) as session:
            deadline = time.monotonic() + 15
            while time.monotonic() < deadline:
                session.read(.02)
                if reply is not None and b"What are you changing?" in session.output:
                    os.write(session.master, reply)
                    reply = None
                if session.poll()[0]:
                    break
            assert session.status is not None, ("product timeout", tool, session.diagnostics())
            session.drain()
            assert reply is None, ("hypothesis prompt never observed", tool, session.output)
            assert os.waitstatus_to_exitcode(session.status) == expected, (tool, session.output, session.status)
            return session.output

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
