"""Real stderr geometry, permanent resize fallback, colors and the unscaled repro."""
import os
from pathlib import Path
import re
import sys
import tempfile
import time
import unicodedata
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "tests"))
from pty_support import PtySession, set_size

binary = str(Path(sys.argv[1]).resolve())
CSI = re.compile(rb"\x1b\[[0-9;?]*[A-Za-z]")


def cells(text):
    # Conservative for the finite builtin deck: ambiguous em dash/ellipsis may
    # take two cells in a CJK terminal. Blocks are the tool's one-cell artwork.
    return sum(2 if c in "—…" or unicodedata.east_asian_width(c) in "WF" else 1 for c in text)


def frames(output):
    return [CSI.sub(b"", line).decode("utf-8")
            for line in re.findall(rb"\r([^\r\n]*?)\x1b\[K", output)]


with tempfile.TemporaryDirectory(prefix="patience-layout-") as temp:
    root = Path(temp).resolve()
    env = dict(os.environ, HOME=str(root), LC_ALL="C", TERM="xterm", NO_COLOR="1",
               PATH="/usr/bin:/bin", PATIENCE_FAST="1", PATIENCE_SEED="7")
    env.pop("PATIENCE_COLOR", None)
    env.pop("PATIENCE_ANGLE", None)

    def run(columns, locale="C", bars=1, rows=24, resize=False, fast=True,
            term="xterm", color=None, no_color="1", exit_code=0):
        child_env = dict(env, LC_ALL=locale, TERM=term)
        if not fast:
            child_env.pop("PATIENCE_FAST", None)
        if no_color is None:
            child_env.pop("NO_COLOR", None)
        else:
            child_env["NO_COLOR"] = no_color
        if color:
            child_env["PATIENCE_COLOR"] = color
        marker = root / "executions"
        marker.write_text("")
        args = [binary, *(["patience"] * (bars - 1)), "sh", "-c",
                'printf "started\\n" >> "$1"; sleep "$2"; printf CHILD_OUT; exit "$3"',
                "child", str(marker), "8" if not fast else ".6", str(exit_code)]
        with PtySession(args, child_env, root, stdout_path=root / "stdout",
                        dimensions=(rows, columns)) as session:
            until = time.monotonic() + (15 if not fast else 5)
            changed_at = None
            stable_frames = None
            while time.monotonic() < until:
                session.read(.01)
                if resize and changed_at is None and len(frames(session.output)) >= bars * 2:
                    set_size(session.slave, rows, 12)
                    changed_at = time.monotonic()
                if changed_at and stable_frames is None and time.monotonic() - changed_at > .12:
                    stable_frames = len(frames(session.output))
                    set_size(session.slave, rows, 120)
                if session.poll()[0]:
                    break
            assert session.status is not None, ("product timeout", session.diagnostics())
            session.drain()
            assert os.waitstatus_to_exitcode(session.status) == exit_code, session.diagnostics()
            lines = frames(session.output)
            expected_animation = term == "xterm" and rows > bars and columns >= 16
            if expected_animation:
                assert lines, session.output
                assert all(cells(line) < columns for line in lines), (columns, lines)
                if resize:
                    assert stable_frames is not None
                    assert len(lines) == stable_frames, "animation restarted after resize"
                    after = session.output.split(b"\x1b[K")[-1]
                    assert b"\x1b[" not in after, "stale-coordinate cleanup after fallback"
                else:
                    peak = max(float(v) for v in re.findall(rb"([0-9]+\.[0-9])%", session.output))
                    assert peak == 100.0 if exit_code == 0 else peak < 100.0
            else:
                assert not lines and b"\x1b" not in session.output, session.output
            if no_color:
                assert not re.search(rb"\x1b\[[0-9;]*m", session.output), "NO_COLOR leaked SGR"
            if color and no_color is None:
                assert re.search(rb"\x1b\[[0-9;]*m", session.output), "missing color fallback"
            assert marker.read_text() == "started\n", "real child ran more than once"
            assert (root / "stdout").read_bytes().startswith(b"CHILD_OUT\n")
            assert b"\x1b[2J" not in session.output, "must not clear unrelated terminal history"
            return lines

    for columns in [20, 24, 40, 80, 120]:
        for locale in ["C", "zh_CN.UTF-8"]:
            for bars in [1, 2]:
                run(columns, locale=locale, bars=bars)
    run(80, rows=2, bars=2)
    run(0)
    run(15)
    run(80, term="dumb")
    run(80, term="unknown")
    run(80, bars=2, resize=True)
    run(40, locale="zh_CN.UTF-8", exit_code=3)
    for color in ["truecolor", "256", "16"]:
        run(80, color=color, no_color=None)
    run(80, color="truecolor", no_color="1")
    # Original review repro: no FAST, Chinese, seed 7, sleep 8, forty columns.
    lines = run(40, locale="zh_CN.UTF-8", fast=False)
    assert len(lines) > 30, "real waiting run was not exercised"
print("patience PTY width/height/resize/colors and unscaled sleep-8 passed")
