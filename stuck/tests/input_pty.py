"""Hypothesis/child stdin boundaries; long canonical failures are opt-in."""
from pathlib import Path
import subprocess
import sys
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "tests"))
from repeat_support import repeat_fixture
from line_input_support import line_cases, check_cases

binary = str(Path(sys.argv[1]).resolve())
with repeat_fixture(binary) as f:
    marker = f.root / "executions"
    cmd = ["sh", "-c", 'printf started >> "$1"; printf CHILD_READY; read reply; printf "child:%s" "$reply"; exit 7',
           "child", str(marker)]
    scenario = 0

    def check(text, chunk, valid):
        global scenario
        scenario += 1
        # New snapshot starts a fresh comparable chain without editing records.
        (f.repo / "code").write_text(str(scenario))
        marker.write_text("")
        for _ in range(3):
            f.run(cmd, code=7, reply=b"child-data\n", trigger=b"CHILD_READY")
        job = f.start(cmd)
        f.wait_for(job, b"What are you changing?", timeout=3)
        # Preserve a second, already queued line for the real child. No prompt
        # reader may read ahead into it. Writes remain bounded on either OS.
        import os
        import time
        payload = text.encode() + b"\n" + (b"child-data\n" if valid else b"")
        os.set_blocking(job.fd, False)
        deadline = time.monotonic() + 3
        while payload and time.monotonic() < deadline:
            try:
                n = os.write(job.fd, payload[:chunk])
                payload = payload[n:]
            except BlockingIOError:
                pass
            job.session.read(0)
        assert not payload, "PTY input injection timed out"
        code, out = f.finish(job, timeout=3)
        assert code == (7 if valid else 125), (code, out[-2000:])
        if valid:
            assert b"child:child-data" in out, "hypothesis reader consumed child stdin"
        else:
            assert b"CHILD_READY" not in out, "invalid hypothesis released the gate"
        assert marker.read_text() == "started" * (4 if valid else 3)
        explicit = subprocess.run([binary, "--hypothesis", text, *cmd], cwd=f.repo,
                                  env=f.env, input=b"child-data\n", capture_output=True, timeout=3)
        assert explicit.returncode == (7 if valid else 2), explicit

    if "--long" in sys.argv:
        check_cases(check, line_cases(512, smoke="--smoke" in sys.argv))
    else:
        check("尝试更短的超时", 65536, True)
        check("   ", 37, False)
