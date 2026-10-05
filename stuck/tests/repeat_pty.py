"""Tool-owned foreground repeat-policy scenarios."""
from pathlib import Path
import subprocess
import sys
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "tests"))
from repeat_support import repeat_fixture, check_wrapper_contract

binary = sys.argv[1]
with repeat_fixture(binary, default_trigger=b"What are you changing?") as f:
    root, repo, env = f.root, f.repo, f.env
    run, start, finish = f.run, f.start, f.finish
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

    check_wrapper_contract(f)
    for path in f.state.rglob("*.json"):
        assert "printf failed" not in path.read_text()
