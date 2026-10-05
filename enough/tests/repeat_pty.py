"""Tool-owned foreground repeat-policy scenarios."""
from pathlib import Path
import sys
import time
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "tests"))
from repeat_support import repeat_fixture, check_wrapper_contract

binary = sys.argv[1]
with repeat_fixture(binary) as f:
    root, repo, env = f.root, f.repo, f.env
    run, start, finish = f.run, f.start, f.finish
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
    check_wrapper_contract(f)
