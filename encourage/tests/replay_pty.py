"""Installed CLI real-chain PTY checks; no production environment hooks."""
import fcntl
import os
from pathlib import Path
import re
import signal
import subprocess
import sys
import tempfile
import termios
import time
import unicodedata

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "tests"))
from pty_support import PtySession, assert_restored, set_size

binary = str(Path(sys.argv[1]).resolve())
ENTER = b"\x1b[?1049h"
LEAVE = b"\x1b[?1049l"
SHOW = b"\x1b[?25h"
SENTENCE = b"Have some cool water, then slowly stretch your body."


def colored(output):
    return any(re.search(rb"(^|;)(3[0-7]|9[0-7]|38)(;|$)", codes)
               for codes in re.findall(rb"\x1b\[([0-9;]*)m", output))


def rendered_header(output, width):
    """Replay Ratatui's cursor-addressed CSI updates, not concatenated text."""
    rows = [[" "] * width for _ in range(2)]
    x = y = 0
    previous = ""
    for match in re.finditer(r"\x1b\[([0-?]*)[ -/]*([@-~])|([^\x1b])",
                             output.decode("utf-8", errors="replace")):
        parameters, command, char = match.groups()
        if command:
            if parameters == "?1049" and command == "h":
                rows = [[" "] * width for _ in range(2)]
                x = y = 0
            if parameters.startswith("?"):
                continue
            values = [int(value) if value else 0 for value in parameters.split(";")]
            amount = values[0] or 1
            if command in ("H", "f"):
                y = amount - 1
                x = (values[1] or 1) - 1 if len(values) > 1 else 0
            elif command == "G":
                x = amount - 1
            elif command == "d":
                y = amount - 1
            elif command in ("A", "B", "C", "D"):
                x = max(0, x + amount * ((command == "C") - (command == "D")))
                y = max(0, y + amount * ((command == "B") - (command == "A")))
            elif command == "J" and values[0] in (2, 3):
                rows = [[" "] * width for _ in range(2)]
            elif command == "K" and y < 2:
                begin = 0 if values[0] in (1, 2) else min(x, width)
                end = min(x + 1, width) if values[0] == 1 else width
                rows[y][begin:end] = [" "] * (end - begin)
            continue
        if char == "\r":
            x = 0
        elif char == "\n":
            x, y = 0, y + 1
        elif char == "\b":
            x = max(0, x - 1)
        elif char == "\ufe0f":
            if previous in ("♥", "❤"):
                x += 1
        elif unicodedata.category(char).startswith("M"):
            continue
        elif char >= " " and char != "\x7f":
            if x >= width:
                x, y = 0, y + 1
            cells = 2 if unicodedata.east_asian_width(char) in ("W", "F") else 1
            if y < 2 and x < width:
                rows[y][x] = char
                if cells == 2 and x + 1 < width:
                    rows[y][x + 1] = " "
            x += cells
            previous = char
    return ["".join(row) for row in rows]


def run(action, locale="C", no_color="1", size=(30, 100), duration=None):
    termination = {"ctrl-c": signal.SIGINT, "term": signal.SIGTERM,
                   "hup": signal.SIGHUP, "quit": signal.SIGQUIT}.get(action)
    expected = -termination if termination else 0
    sentence = subprocess.check_output([binary, "42"], env=dict(os.environ, LC_ALL=locale)).strip()
    if locale == "C":
        assert sentence == SENTENCE
    completed = b"Final sentence" if locale == "C" else "最终句子".encode()
    pool_label = b"Candidate pool" if locale == "C" else "候选原料".encode()
    pool_word = b"enjoy" if locale == "C" else "好好".encode()
    output_label = b"Current output" if locale == "C" else "当前输出".encode()
    with tempfile.TemporaryDirectory(prefix="encourage-pty-") as temp:
        env = dict(os.environ, HOME=temp, XDG_STATE_HOME=str(Path(temp) / "state"),
                   TERM="xterm", LC_ALL=locale, NO_COLOR=no_color)
        args = [binary, *(["--duration", duration] if duration else []), "42"]
        budget = int(duration[:-1]) if duration else 15
        began = time.monotonic()
        with PtySession(args, env, temp, dimensions=size) as session:
            # Darwin also returns a kernel-private bit after Rust stdout use
            # (including --version); compare the documented mutable I/O modes.
            flag_mask = os.O_NONBLOCK | os.O_APPEND | os.O_ASYNC | os.O_SYNC
            flags = fcntl.fcntl(session.slave, fcntl.F_GETFL) & flag_mask
            started = False
            opening_seen = False
            phase = 0
            stopped = False
            pause_at = None
            resize_at = None
            completed_at = None
            deadline = time.monotonic() + (budget + 5 if action == "automatic" else 6)
            while session.status is None and time.monotonic() < deadline:
                session.read()
                if not started and ENTER in session.output:
                    attrs = termios.tcgetattr(session.slave)
                    assert not attrs[3] & (termios.ECHO | termios.ICANON), attrs
                    assert attrs[3] & termios.ISIG, attrs
                    started = True
                if started:
                    if not opening_seen:
                        visible = re.sub(rb"\x1b\[[0-?]*[ -/]*[@-~]", b"", session.output)
                        hearts = sum(visible.count(heart.encode()) for heart in ("♥", "❤", "💗"))
                        opening_seen = (pool_label in visible and pool_word in visible
                                        and b"%" in visible and hearts >= 3)
                        if not opening_seen:
                            session.poll()
                            continue
                    if action == "cancel" and phase == 0:
                        os.write(session.master, b"q")
                        phase = 1
                    elif termination and phase == 0:
                        if action == "ctrl-c":
                            os.write(session.master, b"\x03")
                        else:
                            os.kill(session.worker, termination)
                        phase = 1
                    elif action == "automatic":
                        if phase == 0 and completed in re.sub(rb"\x1b\[[0-?]*[ -/]*[@-~]", b"", session.output):
                            completed_at = time.monotonic()
                            assert completed_at - began >= budget - .1, (duration, completed_at - began)
                            phase = 1
                        elif phase == 1 and time.monotonic() - completed_at >= .2:
                            # Completion is not an automatic exit or stdout write.
                            assert LEAVE not in session.output
                            os.write(session.master, b"\r")
                            phase = 3
                    elif action in ("confirm", "pause", "cancel-completed"):
                        if phase == 0:
                            if action == "pause":
                                os.write(session.master, b" ")
                                pause_at = time.monotonic()
                                phase = 1
                            else:
                                os.write(session.master, b"\r")
                                phase = 2
                        elif phase == 1 and time.monotonic() - pause_at >= .3:
                            os.write(session.master, b"\r")
                            phase = 2
                        elif phase == 2 and completed in re.sub(rb"\x1b\[[0-?]*[ -/]*[@-~]", b"", session.output):
                            os.write(session.master, b"q" if action == "cancel-completed" else b"\r")
                            phase = 3
                    elif action == "resize":
                        if phase == 0:
                            set_size(session.slave, 12, 40)
                            os.kill(session.worker, signal.SIGWINCH)
                            resize_at = time.monotonic()
                            phase = 1
                        elif phase == 1 and time.monotonic() - resize_at >= .3:
                            set_size(session.slave, 30, 100)
                            os.kill(session.worker, signal.SIGWINCH)
                            resize_at = time.monotonic()
                            phase = 2
                        elif phase == 2 and time.monotonic() - resize_at >= .3:
                            os.write(session.master, b"q")
                            phase = 3
                    elif action == "suspend":
                        if phase == 0:
                            os.write(session.master, b"\x1a")
                            phase = 1
                        elif phase == 1:
                            child, status = session.poll()
                            if child and os.WIFSTOPPED(status):
                                assert_restored(session.slave, session.original)
                                stopped = True
                                os.kill(session.worker, signal.SIGCONT)
                                phase = 2
                        elif phase == 2 and session.output.count(ENTER) >= 2:
                            os.write(session.master, b"q")
                            phase = 3
                session.poll()
            assert session.status is not None, (action, session.diagnostics())
            session.drain()
            output = session.output
            assert opening_seen, (action, "missing labelled opening candidates", output[-2000:])
            assert os.waitstatus_to_exitcode(session.status) == expected, (action, output[-2000:])
            assert_restored(session.slave, session.original)
            assert fcntl.fcntl(session.slave, fcntl.F_GETFL) & flag_mask == flags, "inherited I/O modes changed"
            assert ENTER in output and LEAVE in output and SHOW in output, (action, output[-2000:])
            assert output.rfind(LEAVE) > output.rfind(ENTER), action
            assert b"\x1b[6n" not in output, "cursor position query is forbidden"
            if no_color:
                assert not colored(output), "NO_COLOR must suppress foreground colors"
            else:
                assert colored(output), "default replay must retain Rust highlighting"
            if action in ("confirm", "pause", "automatic"):
                assert phase == 3, (action, phase, output[-2000:])
                tail = output[output.rfind(LEAVE) + len(LEAVE):].strip()
                assert tail == sentence, (action, tail)
            else:
                assert sentence not in output[output.rfind(LEAVE):], action
            if action == "cancel-completed":
                assert phase == 3
            if action == "automatic":
                assert completed_at is not None
                header = rendered_header(output[:output.rfind(LEAVE)], size[1])
                assert output_label.decode() in header[1], header
                assert "100%" in header[0], header
            if action == "suspend":
                assert stopped and output.count(ENTER) >= 2, action
            assert not (Path(temp) / "state").exists()


def plain_fallbacks():
    with tempfile.TemporaryDirectory(prefix="encourage-plain-pty-") as temp:
        for term, size, redirect, args, code in (
            ("dumb", (30, 100), False, ["42"], 0),
            ("", (30, 100), False, ["42"], 0),
            ("xterm", (12, 40), False, ["42"], 0),
            ("xterm", (30, 100), True, ["42"], 0),
            ("xterm", (30, 100), True, ["--duration", "600s", "42"], 0),
            ("dumb", (30, 100), False, ["--duration", "600s", "42"], 0),
            ("xterm", (30, 100), False, ["--duration", "9s", "42"], 2),
            ("xterm", (30, 100), False, ["--help"], 0),
            ("xterm", (30, 100), False, ["--version"], 0),
            ("xterm", (30, 100), False, ["--unknown"], 2),
        ):
            env = dict(os.environ, HOME=temp, TERM=term, LC_ALL="C")
            path = Path(temp) / "stdout" if redirect else None
            with PtySession([binary, *args], env, temp, dimensions=size, stdout_path=path) as session:
                deadline = time.monotonic() + 3
                while session.status is None and time.monotonic() < deadline:
                    session.read()
                    session.poll()
                assert session.status is not None, session.diagnostics()
                session.drain()
                assert os.waitstatus_to_exitcode(session.status) == code, session.diagnostics()
                assert_restored(session.slave, session.original)
                assert ENTER not in session.output and LEAVE not in session.output
                if code == 0 and args[-1] == "42":
                    result = path.read_bytes() if path else session.output.replace(b"\r", b"")
                    assert result == SENTENCE + b"\n", result


# Shared prefixes and unchanged '%' cells must survive incremental updates.
assert rendered_header(b"\x1b[2;1HCandidate pool\x1b[2;2Hurrent output", 80)[1].strip() == "Current output"
assert "100%" in rendered_header(b"\x1b[1;5H 99%\x1b[1;5H100", 80)[0]

for scenario in ("confirm", "pause", "automatic", "cancel", "cancel-completed", "ctrl-c", "term", "hup", "quit", "resize", "suspend"):
    run(scenario)
run("automatic", duration="10s")
run("automatic", duration="17s")
run("confirm", duration="600s")
run("confirm", "zh_CN.UTF-8")
run("confirm", no_color="", size=(24, 80))
run("automatic", no_color="", size=(24, 80), duration="10s")
plain_fallbacks()
print("encourage CLI real-chain PTY lifecycle and plain fallbacks passed")
