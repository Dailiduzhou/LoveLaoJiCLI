"""Tool-owned interactive save/no-overwrite checks, including opt-in long input."""
from pathlib import Path
import subprocess
import sys
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "tests"))
from foreground_support import foreground_fixture
from line_input_support import line_cases, check_cases

binary = str(Path(sys.argv[1]).resolve())
with foreground_fixture(binary) as f:
    def command(*args):
        result = subprocess.run([binary, *args], cwd=f.root, env=f.env,
                                capture_output=True, timeout=3)
        assert result.returncode == 0, result
        return result.stdout

    def check(text, chunk, valid):
        command("--next", "original context")
        card = next((f.root / "state").rglob("later.json"))
        before = card.read_bytes()
        try:
            f.run([], [(b"What is the very next thing?", text.encode() + b"\n")],
                  expected=0 if valid else 125, timeout=3, write_chunk=chunk)
        except BaseException:
            # A timed-out/failed input must still leave the old card intact.
            assert card.read_bytes() == before
            raise
        if valid:
            assert command("resume").startswith(f"Next: {text}\n\n".encode())
        else:
            assert card.read_bytes() == before

    if "--long" in sys.argv:
        check_cases(check, line_cases(2000, smoke="--smoke" in sys.argv))
    else:
        check("继续一个小步骤", 37, True)
        check(" " * 3, 37, False)
        command("--next", "keep this")
        card = next((f.root / "state").rglob("later.json"))
        before = card.read_bytes()
        f.run([], [(b"What is the very next thing?", b"\x04")], expected=125)
        assert card.read_bytes() == before
