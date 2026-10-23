"""Known failing canonical-input regression; run explicitly, never call it fixed."""
from pathlib import Path
import subprocess
import sys
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "tests"))
from foreground_support import foreground_fixture
from line_input_support import line_cases, check_cases

binary = str(Path(sys.argv[1]).resolve())
with foreground_fixture(binary) as f:
    rest = ["--actual", "actual", "--last-change", "change", "--experiment", "experiment"]

    def check(text, chunk, valid):
        explicit = subprocess.run([binary, "--expected", text, *rest], env=f.env,
                                  cwd=f.root, capture_output=True, timeout=3)
        assert explicit.returncode == (0 if valid else 2), explicit
        out, _, _ = f.run(rest, [(b"Expected?", text.encode() + b"\n")],
                          expected=0 if valid else 125, timeout=3, write_chunk=chunk)
        assert out == (explicit.stdout if valid else b""), "truncated or partial summary"
        assert not (f.root / "state").exists()

    check_cases(check, line_cases(2000, smoke="--smoke" in sys.argv))
