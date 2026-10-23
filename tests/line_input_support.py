"""Parameterized generated text for the shared single-line input contract.

These cases intentionally include known canonical-buffer failures. Do not shorten
input or treat timeouts/truncation as success. Tool adapters own prompts, flags,
state checks and child-input checks. No answers or private state are persisted.
"""
import os
import pty


def line_cases(limit, smoke=False):
    yield "600-cjk-paste", "中" * 600, 65536, 600 <= limit
    if smoke:
        return
    # Exercise actual platform boundaries without assuming PC_MAX_CANON is the
    # full kernel implementation limit (Linux commonly reports 255, uses 4096).
    master, slave = pty.openpty()
    try:
        canon = os.fpathconf(slave, "PC_MAX_CANON")
    finally:
        os.close(master)
        os.close(slave)
    for boundary in sorted({canon, 1024, 4096}):
        for length in [boundary - 1, boundary, boundary + 1]:
            yield f"ascii-buffer-{length}", "x" * length, 65536, length <= limit
    for name, alphabet in [("ascii", "x"), ("cjk", "中"), ("mixed", "中x")]:
        for length in [limit - 1, limit, limit + 1]:
            text = (alphabet * length)[:length]
            for chunk in [65536, 37]:
                yield f"{name}-{length}-chunk-{chunk}", text, chunk, length <= limit


def check_cases(check, cases):
    failures = []
    for name, text, chunk, valid in cases:
        try:
            check(text, chunk, valid)
        except (AssertionError, TimeoutError) as error:
            failures.append(name)
            print(f"FAIL {name}: {error}", flush=True)
        else:
            print(f"PASS {name}", flush=True)
    assert not failures, f"unresolved line-input contract: {', '.join(failures)}"
