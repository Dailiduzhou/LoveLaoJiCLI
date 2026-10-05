"""Real foreground PTYs, not environment variables that bypass safety checks."""
import errno
import os
from pathlib import Path
import pty
import select
import signal
import subprocess
import tempfile
import time

from contextlib import contextmanager
from types import SimpleNamespace

@contextmanager
def repeat_fixture(binary, default_trigger=None):
    tool = Path(binary).name
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

        def finish(job, reply=None, trigger=default_trigger, timeout=15):
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
                    if reply is not None and trigger is not None and trigger in output:
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

        yield SimpleNamespace(root=root, repo=repo, state=state, env=env, run=run, start=start, finish=finish)

def check_wrapper_contract(f):
    run, start, finish, state = f.run, f.start, f.finish, f.state
    # Child stdin remains inherited; a hypothesis prompt never reads it.
    out = run(["sh", "-c", "printf ready; read x; printf 'child:%s' \"$x\""], reply=b"hello\n", trigger=b"ready")
    assert b"child:hello" in out, out
    # Ctrl+C reaches the shared foreground process group. No success is recorded.
    code, out = finish(start(["sh", "-c", "echo ready; sleep 30"]), reply=b"\x03", trigger=b"ready")
    assert code in (-signal.SIGINT, 128 + signal.SIGINT), (code, out)
    # Original shell program text and output are never persisted.
    for path in state.rglob("*.json"):
        content = path.read_text()
        assert "sleep 30" not in content
