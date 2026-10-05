"""Tool-owned foreground prompt/plain-output scenarios."""
from pathlib import Path
import signal
import sys
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "tests"))
from foreground_support import foreground_fixture

binary = sys.argv[1]
with foreground_fixture(binary) as f:
    root, env, run = f.root, f.env, f.run
    questions = [(b"Expected?", b"success\n"), (b"Actual?", b"failure\n"),
                 (b"Last change?", b"timeout\n"), (b"Next experiment?", b"increase it\n")]
    expected = b"Expected: success\nActual: failure\nLast change: timeout\nNext experiment: increase it\nquack.\n"
    out, prompts, _ = run([], questions)
    assert out == expected, out
    for question, _ in questions:
        assert question in prompts
        assert question not in out
    out, prompts, _ = run(["--expected", "success", "--last-change", "timeout"],
                          [questions[1], questions[3]])
    assert out == expected
    assert b"Expected?" not in prompts and b"Last change?" not in prompts
    for answer in [b"\x04", b"   \n", b"x" * 2001 + b"\n"]:
        out, _, _ = run([], [(b"Expected?", answer)], expected=125)
        assert out == b""
    out, _, _ = run([], cancel="ctrl-c", expected=-signal.SIGINT)
    assert out == b""
    out, prompts, _ = run(["--expected", "success", "--actual", "failure",
                           "--last-change", "timeout", "--experiment", "increase it"])
    assert out == expected and prompts == b""
    assert not (root / "state").exists()
