"""Real foreground PTYs, not environment variables that bypass safety checks."""
import errno
import json
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
        env = {k: v for k, v in os.environ.items() if not k.startswith("GIT_")}
        env.update(HOME=temp, XDG_STATE_HOME=str(state), XDG_CONFIG_HOME=str(root / "config"),
                   PATH="/usr/bin:/bin", LC_ALL="C", GIT_CONFIG_GLOBAL="/dev/null", GIT_CONFIG_NOSYSTEM="1")
        for k in ["BASH_ENV", "ENV"]:
            env.pop(k, None)
        def git(*args):
            subprocess.run(["git", *args], cwd=repo, env=env, check=True, capture_output=True)
        git("init", "-q")
        git("config", "user.name", "Test")
        git("config", "user.email", "test@localhost")
        (repo / "code").write_text("one\n")
        git("add", ".")
        git("commit", "-qm", "init")

        jobs = []

        def start(args):
            pid, fd = pty.fork()
            if pid == 0:
                os.chdir(repo)
                os.execve(binary, [binary, *args], env)
            job = SimpleNamespace(pid=pid, fd=fd, output=b"", status=None, closed=False)
            jobs.append(job)
            return job

        def read(job, delay=.05):
            if not select.select([job.fd], [], [], delay)[0]:
                return True
            try:
                data = os.read(job.fd, 65536)
            except OSError as error:
                if error.errno == errno.EIO:
                    return False
                raise
            job.output += data
            return bool(data)

        def poll(job):
            if job.status is None:
                child, status = os.waitpid(job.pid, os.WNOHANG)
                if child:
                    job.status = status
            return job.status

        def close(job):
            if job.closed:
                return
            if job.status is None:
                try:
                    os.killpg(job.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                _, job.status = os.waitpid(job.pid, 0)
            os.close(job.fd)
            job.closed = True

        def wait_for(job, trigger, timeout=15):
            deadline = time.monotonic() + timeout
            while trigger not in job.output:
                assert time.monotonic() < deadline, ("missing readiness", tool, job.output)
                read(job)
                assert poll(job) is None, ("exited before readiness", tool, job.output)
            return job.output

        def finish(job, reply=None, trigger=default_trigger, timeout=15):
            deadline = time.monotonic() + timeout
            try:
                while time.monotonic() < deadline:
                    open_pty = read(job)
                    if reply is not None and trigger is not None and trigger in job.output:
                        os.write(job.fd, reply)
                        reply = None
                    if poll(job) is not None:
                        while select.select([job.fd], [], [], 0)[0] and read(job, 0):
                            pass
                        break
                    if not open_pty:
                        time.sleep(.01)
                else:
                    raise AssertionError(f"hung {tool}: {job.output!r}")
                assert reply is None, ("reply trigger was never observed", trigger, job.output)
                return os.waitstatus_to_exitcode(job.status), job.output
            finally:
                close(job)

        def run(args, code=0, **kw):
            result = finish(start(args), **kw)
            assert result[0] == code, result
            return result[1]

        try:
            yield SimpleNamespace(root=root, repo=repo, state=state, env=env,
                                  run=run, start=start, finish=finish, wait_for=wait_for)
        finally:
            for job in jobs:
                close(job)

def check_wrapper_contract(f):
    run, start, finish, state = f.run, f.start, f.finish, f.state
    # Child stdin remains inherited; a hypothesis prompt never reads it.
    out = run(["sh", "-c", "printf ready; read x; printf 'child:%s' \"$x\""], reply=b"hello\n", trigger=b"ready")
    assert b"child:hello" in out, out
    # Ctrl+C reaches the shared foreground process group. No success is recorded.
    before = set(state.rglob("runs/*.json"))
    code, out = finish(start(["sh", "-c", "echo ready; sleep 30"]), reply=b"\x03", trigger=b"ready")
    assert code in (-signal.SIGINT, 128 + signal.SIGINT), (code, out)
    interrupted = set(state.rglob("runs/*.json")) - before
    assert len(interrupted) == 1, interrupted
    path = interrupted.pop()
    record = json.loads(json.loads(path.read_text())["payload"])
    assert record["started_at"] is not None
    assert record["finished_at"] is None and record["result"] is None, record
    repeat_path = path.parent.parent / (record["tool"] + ".json")
    repeat = json.loads(json.loads(repeat_path.read_text())["payload"])
    assert repeat["baseline"] is None, "interrupted run retained a usable baseline"
    # Original shell program text and output are never persisted.
    for path in state.rglob("*.json"):
        content = path.read_text()
        assert "sleep 30" not in content
