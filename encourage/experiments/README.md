# encourage: P0 generation experiments

[简体中文](README-zh.md)

This document covers only the isolated Rust library in this directory. The lab provides no installable CLI or TUI and is not a root workspace member. This round did not change the installer, existing tools, or terminal dependency policy. A test keeps its package version aligned with the parent workspace.

The parent `encourage/src/`, `Cargo.toml`, and tool-level READMEs contain concurrent TUI development, neither created nor validated in this round. The lab does not depend on that crate; these results do not certify its TUI.

**Under the latest request, the chain now executes 34 operations: 28 content operations plus 6 explicitly marked reversible transitions.** This meets the revised total-stage range, not a claim of 34 effective semantic transformations. `src/pipeline.rs` retains the unedited 12-operation baseline; the current chain is in `src/expanded.rs` and lexical edits are in `src/revision.rs`. Nested helpers, `inspect`, and observation counts remain excluded.

## Reproduce

Run from the repository root (`--offline` requires locally cached dependencies):

```sh
cargo test --locked --offline --manifest-path encourage/experiments/Cargo.toml
cargo test --locked --offline --release --manifest-path encourage/experiments/Cargo.toml
cargo clippy --locked --offline --manifest-path encourage/experiments/Cargo.toml --all-targets -- -D warnings
cargo fmt --manifest-path encourage/experiments/Cargo.toml -- --check
cargo run --locked --offline --quiet --manifest-path encourage/experiments/Cargo.toml --example survey
```

The last command is a development report, not a production CLI; it accepts no business arguments. This lab uses only std and the already permitted `blake3`, with no new terminal, signal, or parser dependencies. Root `cargo test --workspace` does not automatically include this isolated lab.

[Reproducible observations](observations.md) contain every Chinese and English combination, seed vectors, counts, and actual execution events. Regenerate with:

```sh
cargo run --locked --offline --quiet --manifest-path encourage/experiments/Cargo.toml --example survey > encourage/experiments/observations.md
```

## Round three: reversible transitions, then real edits

The user explicitly allowed presentation no-ops. This changes the earlier requirement that every step alter content; no-op transitions are not disguised as semantic work.

| Stages | Kind | Behavior |
| --- | --- | --- |
| 19–20 | Transition | `zip(0_u64..)` with display ordinals, then `map` back to fragments |
| 21–22 | Transition | `map(std::iter::once)` into singleton iterators, then `flatten` back without swallowing `Err` |
| 23–24 | Transition | Fixed-seed local Fisher–Yates shuffle, then restore original-position order |
| 25 | Content | Expand restored fragments into atoms |
| 26 | Content | Insert actual new words before the original anchor, retaining that original temporarily |
| 27 | Content | Delete the original anchored word, never the inserted words |
| 28–34 | Content | End boundary, positions/budgets, localized formatting, and `fold` |

Each bounded fragment gets a separate `SplitMix64(seed=2)` and unbiased bounded sampling. Shuffle neither reads OS entropy nor borrows the generation RNG. Empty and singleton fragments remain unchanged. Position tags and origins survive the shuffle: restoration cannot rely on text equality, and `End` must return to its original position before `take_while`. A known pair is `[enjoy, a] → [a, enjoy] → [enjoy, a]`.

`Generation.draws` counts theme, structure, and slot-selection words only, **excluding the independent shuffle RNG**. It is not the total number of random calls in the process. Pausing, preview caps, or future rendering must not alter either stream's rules.

Real edits match language, frame, and fixed-slot/suffix location, not whole-text replacement or input sentiment. Every successful run must hit one insertion anchor and remove one original word. `Inserted` provenance prevents the deletion filter from removing newly inserted text. Examples:

```text
好好吃饭，慢慢照顾自己。
→ 好好吃饭，慢慢善待自己。

Have some warm water and gently stretch your body.
→ Have some warm water, then gently stretch your body.

Give yourself credit for the small steps you take.
→ Give yourself credit for the small steps you have taken.
```

The insertion stage temporarily contains both new and old words and may not be grammatical yet; that is a real intermediate observation, not a claim that every frame is a complete sentence. Final output still has 34 distinct outcomes per language. Byte and atom limits apply after the edits.

## Round-two record: 26-operation expansion

At that stage, the expanded version preserved the corpus, argv-to-seed mapping, and draw schedule. The new editorial step intentionally changes final text, so equality with the old baseline's sentence is no longer the right assertion. Theme and structure choices, formerly outside the chain, now execute in two genuine main-chain `map` operations. Other additions expose catalogue selection, slot compatibility, and word-rule expansion that were previously direct lookups.

| Operations | Data and responsibility |
| --- | --- |
| 1–2 | Random words → theme and structure choices |
| 3–8 | Bilingual catalogue → theme → structure, using successive `flat_map` / `filter` operations |
| 9–14 | Frame → slot IDs → frame-local candidate bank → role filtering → `scan` grouping → `flatten` completed groups |
| 15–16 | `zip` each complete candidate group with its first random word; retry within that group |
| 17–19 | Fixed prefix / chosen rule / fixed suffix → individual atoms with distinct origins |
| 20–26 | End boundary, positions, localized punctuation, spacing, capitalization, final `fold` |

The filters actually change the stream. A meal example rejects one other language catalogue, three other themes, and one other structure. Querying the frame's real candidate bank for both slots rejects five wrong-role candidates. Seven candidate/boundary events become two complete groups; boundaries and incomplete groups do not become output words.

The two versions zip different element types:

- The 12-operation version streams individual sampling attempts, so rejected attempts must be removed before pairing with slots.
- The 26-operation version pairs a **complete candidate group** with its first random word. A rejection keeps the same group and draws another word; it neither advances the left slot stream nor filters out the entire zipped group.

Injected consecutive rejections verify that slots are not skipped or misaligned and that no next-slot word is over-consumed. Fixed prefixes now carry `Fixed` provenance, only selected alternatives carry `Choice { slot, draw }`, and the fixed suffix carries `Suffix`; fixed words are not falsely attributed to random choices.

Catalogue traversal and grouping do real extra work compared with direct indexing. This makes rule selection visible; it is not a performance claim. The implementation is one continuous main chain. Local bounded materialization, helper-internal iterators, and tracing do not add operations to the count.

## Round-one baseline combination

```text
Length-prefixed argv encoding preserving argument boundaries
  → BLAKE3 fingerprint with a fixed public key
  → SplitMix64 word stream
  → uniform theme / structure choices
  → rejection sampling for each slot's candidate count
  → filter rejected words, then zip with slots
  → select and expand word-level rules
  → localized punctuation / spacing / initial English capitalization
  → fold into a sentence
```

- Input determines the seed, not an interpreted emotion. The public key and seed provide no input confidentiality.
- Slot-local candidates express compatibility. There is no arbitrary selection from an untyped pool or selection of a complete sentence followed by artificial splitting.
- Fixed text and alternative rules consist of short words; complete sentences appear only in documentation, reports, and test expectations.
- Theme and structure selection precedes the language branch, preserving bilingual intent for the same seed.
- One real slot has three alternatives, the others currently two. Four draws per run in the normal sweep is not a permanent consumption guarantee: rejection adds draws.
- Choices are uniform within each layer, not globally over final sentences. A structure with six combinations is not selected more often merely because another has four.

## Sentence experiment

Each language has four themes and eight structures: six combinations for `body.meal`, four for each other structure, or **34 sentences per language, 68 total**. Exhaustive tests and the report cover every combination. An initial editorial reading was performed, but release-level editorial approval remains necessary; tests cannot establish natural language quality.

Combination review prompted changes to the sketch:

- Add a third meal alternative, `安心` / `hearty`, so three-candidate validation is not limited to synthetic fixtures.
- Replace the underspecified `今天完成一点` with `今天有一点{进展|收获}`.
- Remove `眼前的` from the cumbersome `先走出眼前的这一步`.
- Prefer `a short break` to `a gentle break`.
- Do not freely substitute `pace` / `rhythm` after a fixed `at`. Expand word-level rules `at + your + own + pace` and `to + your + own + rhythm` instead of storing complete clauses.

```text
安心吃饭，用心善待自己。
今天有一点进展，就送给自己一点鼓励。
Enjoy a hearty meal and take a short pause.
Let yourself take a breath; you do not have to rush.
```

Untyped control: pool both variable slots' alternatives for each frame and try every pair. Of 137 assignments per language, only 34 satisfy the declared slot contracts; typed slots directly produce 34 compatible outcomes. **This measures contract compliance, not a claim that all remaining 103 assignments are ungrammatical.**

## RNG and iterator evidence

The current 40 tests cover the baseline, expanded chain, three reversible transition pairs, and lexical edits:

- Length-prefixed UTF-8, argument boundaries, explicit empty input, no Unicode normalization, and six argv-to-seed golden vectors.
- Known SplitMix64 vectors and an independent widened `u128` oracle for wrapping arithmetic.
- Exhaustive eight-bit models for bounds 1–255: with `n=3`, direct modulo counts `[86,85,85]`; rejecting the excess interval gives `[85,85,85]`.
- Injecting `[0,0,0,3,1]` into the real corpus: after two header choices, the three-candidate slot rejects `0`, accepts `3`, then advances without losing or misaligning a slot.
- Across 20,000 bilingual generations over seeds `0..10000`, edited sentences agree with an independent imperative editorial oracle. Frame choices and generation draw counts still match the unedited baseline; full sentences intentionally differ. All 34 outcomes per language are reached.
- All 68 combinations verify real insertion, deletion of one original word, preservation of inserted origins, and final spacing/punctuation; byte and atom limits include inserted text.
- A known fixed-seed shuffle, exact restoration of every fragment and end marker, empty fragments, and error preservation; zip/singleton round-trip snapshots match their inputs.
- All 34 source markers distinguish 28 content and 6 transition operations. Real candidate filtering, grouping, and provenance remain covered.
- `take_while` consumes the stopping item but not the tail; `zip` can pull an extra unpaired left item.
- Moving rejection filtering after `zip` corrupts the baseline's attempt/slot alignment. The expanded version instead pairs whole groups and retries locally; tests contrast both correct designs with the incorrect ordering.
- Actual execution interleaves per item: the first `fold` occurs before the second slot's random draw. Stage replay is not line-by-line execution.
- `fold(Result<...>)` preserves an error but continues pulling; `try_fold` short-circuits. This lab retains the agreed `fold`, bounds its input, and narrowly documents the Clippy exception rather than claiming automatic failure short-circuiting.
- Tracing and preview limits preserve output and consumption. Draw, atom, or byte exhaustion returns an error, not a backup sentence.

Across 10,000 seeds, structure counts are `[1206,1209,1254,1263,1264,1316,1241,1247]` in both languages. Individual sentence counts range from 186 to 360. These are diagnostics for layered selection with different combination counts, not frequency-based correctness thresholds.

## Boundaries and next step

On **macOS arm64 with rustc 1.99.1**, all 40 tests passed in both debug and release, along with Clippy, formatting, and byte-for-byte report reproduction. Linux/WSL and minimum-Rust-version execution have not been validated.

Not implemented or validated by this lab: no-argument OS entropy, production CLI/locale wiring, TUI, terminal restoration, installation, or a frozen generation protocol. This experimental corpus and draw schedule are not a released `encourage-gen-v1` contract.

Under the revised policy permitting explicitly marked transitions, 34 total operations fall within 32–36; only 28 are content operations, so this does not satisfy the former requirement of 32 effective semantic operations. The independent editorial oracle verifies new text; the 12-operation baseline still verifies unedited text and selection RNG behavior. Release-level editorial approval, concurrent TUI integration, and generation-protocol freezing remain outstanding. Observation counts and separate prototypes are never added to the operation count.

See the root [plan.md](../../plan.md) (Chinese design document) for the target.
