"""Real foreground PTYs, not environment variables that bypass safety checks."""
import errno
import json
import os
from pathlib import Path
import pty
import select
import signal
import subprocess
import sys
import tempfile
import time

binary, tool = sys.argv[1:]
with tempfile.TemporaryDirectory(prefix="humanutils-pty-") as temp:
    root = Path(temp)
    repo = root / "repo"
    repo.mkdir()
    state = root / "state"
    state.mkdir(mode=0o700)
    env = dict(os.environ, HOME=temp, XDG_STATE_HOME=str(state), LC_ALL="C",
               GIT_CONFIG_GLOBAL="/dev/null", GIT_CONFIG_NOSYSTEM="1")
    for k in ["GIT_DIR", "GIT_WORK_TREE", "GIT_INDEX_FILE", "GIT_COMMON_DIR", "GIT_CONFIG_COUNT", "GIT_CONFIG_PARAMETERS"]:
        env.pop(k, None)
    def git(*args):
        subprocess.run(["git", *args], cwd=repo, env=env, check=True, capture_output=True)
    git("init", "-q")
    git("config", "user.name", "Test")
    git("config", "user.email", "test@localhost")
    (repo / "code").write_text("one\n")
    git("add", ".")
    git("commit", "-qm", "init")

    def start(args):
        pid, fd = pty.fork()
        if pid == 0:
            os.chdir(repo)
            os.execve(binary, [binary, *args], env)
        return pid, fd

    def finish(job, reply=None, trigger=b"What are you changing?", timeout=15):
        pid, fd = job
        output = b""
        deadline = time.monotonic() + timeout
        try:
            while time.monotonic() < deadline:
                ready, _, _ = select.select([fd], [], [], .05)
                if not ready:
                    continue
                try:
                    data = os.read(fd, 65536)
                except OSError as e:
                    if e.errno == errno.EIO:
                        break
                    raise
                if not data:
                    break
                output += data
                if reply is not None and trigger in output:
                    os.write(fd, reply)
                    reply = None
            else:
                os.killpg(pid, signal.SIGKILL)
                raise AssertionError(f"hung {tool}: {output!r}")
        finally:
            os.close(fd)
        _, status = os.waitpid(pid, 0)
        return os.waitstatus_to_exitcode(status), output

    def run(args, code=0, **kw):
        result = finish(start(args), **kw)
        assert result[0] == code, result
        return result[1]

    if tool == "enough":
        cmd = ["sh", "-c", "printf ran"]
        assert b"ran" in run(cmd)
        for message in [b"It already passed.", b"Nothing changed.", b"enough.", b"no.", b"no."]:
            out = run(cmd)
            assert message in out and b"ran" not in out, out
        assert b"ran" in run(["--again", *cmd])
        (repo / "code").write_text("two\n")
        assert b"ran" in run(cmd)
        assert b"already passed" in run(cmd)
        # Same argv, external outcome changes: --again failure clears the old success.
        gate = root / "gate"
        gate.write_text("ok")
        cmd = ["sh", "-c", f"test -f '{gate}'; exit $?"]
        run(cmd)
        gate.unlink()
        run(["--again", *cmd], code=1)
        run(cmd, code=1)
        # Overlap: neither result becomes a reliable baseline.
        cmd = ["sh", "-c", "echo ran; sleep .8"]
        a = start(cmd)
        time.sleep(.25)
        b = start(cmd)
        assert finish(a)[0] == 0
        assert finish(b)[0] == 0
        assert b"ran" in run(cmd)
        assert b"already passed" in run(cmd)
    else:
        cmd = ["sh", "-c", "printf failed; printf err >&2; exit 7"]
        for _ in range(3):
            assert b"failed" in run(cmd, code=7)
        out = run(cmd, code=125, reply=b"\x04")
        assert b"failed" not in out and b"Nothing changed" in out, out
        out = run(cmd, code=7, reply=b"try a different timeout\n")
        assert b"failed" in out, out
        for _ in range(2):
            run(cmd, code=7)
        run(["--hypothesis", "increase timeout", *cmd], code=7)
        for _ in range(2):
            run(cmd, code=7)
        (repo / "code").write_text("changed\n")
        assert b"Nothing changed" not in run(cmd, code=7)
        # Different output resets the chain (argv deliberately stays identical).
        outside = root / "outside"
        outside.write_text("a")
        cmd = ["sh", "-c", f"cat '{outside}'; exit 8"]
        run(cmd, code=8)
        run(cmd, code=8)
        outside.write_text("b")
        for _ in range(3):
            run(cmd, code=8)
        run(cmd, code=125, reply=b"\n")
        # A pipe must never be stolen for a hypothesis; incomparable input runs.
        r = subprocess.run([binary, *cmd], cwd=repo, env=env, input=b"stdin-data", capture_output=True, timeout=10)
        assert r.returncode == 8 and r.stdout == b"b", r

    # Child stdin remains inherited; a hypothesis prompt never reads it.
    out = run(["sh", "-c", "printf ready; read x; printf 'child:%s' \"$x\""], reply=b"hello\n", trigger=b"ready")
    assert b"child:hello" in out, out
    # Ctrl+C reaches the shared foreground process group. No success is recorded.
    code, out = finish(start(["sh", "-c", "echo ready; sleep 30"]), reply=b"\x03", trigger=b"ready")
    assert code in (-signal.SIGINT, 128 + signal.SIGINT), (code, out)
    # Original shell program text and output are never persisted.
    for path in state.rglob("*.json"):
        content = path.read_text()
        assert "printf failed" not in content and "sleep 30" not in content
