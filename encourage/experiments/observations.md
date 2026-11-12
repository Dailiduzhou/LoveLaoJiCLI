# Encourage P0 reproducible observations

Experimental corpus and consumption order; NOT a frozen encourage-gen-v1 release.

## Seed mapping

| argv | seed (hex) |
| --- | --- |
| `[""]` | `f27173f52ec47b20` |
| `["hello"]` | `06fbe595ebd1a095` |
| `["今天有点累"]` | `604617da3b6772e8` |
| `["a", "b"]` | `77b0793e49468a01` |
| `["a b"]` | `6d0a032117dfae19` |
| `["42"]` | `b6afcb860bc20ce9` |

SplitMix64(seed=0): `[e220a8397b1dcdaf, 6e789e6aa1b965f4, 06c45d188009454f, f88bb8a8724c81ec, 1b39896a51a8749b]`

## Compact chain

12 actual main-chain operations; target 32–36 NOT met. Header choices and nested helpers are excluded.

| # | Adapter | Purpose |
| --- | --- | --- |
| 1 | `filter` | reject out-of-range random words without advancing a slot |
| 2 | `zip` | pair accepted tickets with their ordered grammar slots |
| 3 | `map` | select the indexed compatible lexical alternative |
| 4 | `flat_map` | expand fixed prefix and selected word-level rule |
| 5 | `chain` | append the grammar suffix and its end boundary |
| 6 | `take_while` | consume the end boundary, never emit it as text |
| 7 | `enumerate` | assign stable output positions and enforce atom limits |
| 8 | `map` | resolve words and language-specific punctuation |
| 9 | `scan` | derive word spacing from prior emitted content |
| 10 | `flat_map` | expand spacing decisions into text pieces |
| 11 | `scan` | capitalize only the first English word |
| 12 | `fold` | assemble the sentence under the byte budget |

## Expanded chain: explicit transitions then real edits

34 executed operations = 28 content + 6 reversible transitions. The new requested transition policy permits these no-ops; they are NOT described as content transformations. Nested helpers and inspect remain excluded.

| # | Kind | Adapter | Purpose |
| --- | --- | --- | --- |
| 1 | content | `map` | derive the theme from the first random word |
| 2 | content | `map` | draw the structure within the chosen theme |
| 3 | content | `flat_map` | open the bilingual corpus catalogues |
| 4 | content | `filter` | select the requested language catalogue |
| 5 | content | `flat_map` | open the four theme groups |
| 6 | content | `filter` | select the sampled theme |
| 7 | content | `flat_map` | open the theme's two structures |
| 8 | content | `filter` | select the sampled structure |
| 9 | content | `flat_map` | expand the selected frame into grammar slots |
| 10 | content | `enumerate` | assign slot ordinals used for candidate ownership |
| 11 | content | `flat_map` | query the frame's candidate bank for each requested slot |
| 12 | content | `filter` | retain only candidates owned by that grammar slot |
| 13 | content | `scan` | group compatible candidates until the slot boundary |
| 14 | content | `flatten` | emit completed candidate groups, not partial groups |
| 15 | content | `zip` | pair each complete slot with its first random word |
| 16 | content | `map` | sample within the compatible group, retrying in the same slot |
| 17 | content | `flat_map` | emit fixed-prefix and chosen-rule fragments with distinct origins |
| 18 | content | `chain` | append the selected frame's fixed suffix |
| 19 | transition | `zip` | TRANSITION: pair each fragment with a display ordinal |
| 20 | transition | `map` | TRANSITION: discard the ordinal and restore the fragment |
| 21 | transition | `map` | TRANSITION: wrap each fragment in a singleton iterator |
| 22 | transition | `flatten` | TRANSITION: unwrap the singleton without losing errors |
| 23 | transition | `map` | TRANSITION: Fisher-Yates shuffle with independent fixed seed |
| 24 | transition | `map` | TRANSITION: restore atoms by their original positions |
| 25 | content | `flat_map` | expand restored fragments into word and boundary atoms |
| 26 | content | `flat_map` | insert actual editorial words before their original anchor |
| 27 | content | `filter` | remove the original anchored word, never the inserted words |
| 28 | content | `take_while` | consume the sentence-end boundary without rendering it |
| 29 | content | `enumerate` | assign output positions for the atom budget |
| 30 | content | `map` | resolve words and locale-specific punctuation |
| 31 | content | `scan` | derive word spacing from prior emitted content |
| 32 | content | `flat_map` | expand spacing decisions into text pieces |
| 33 | content | `scan` | capitalize the first English word only |
| 34 | content | `fold` | assemble the final sentence under the byte budget |

Shuffle uses independent SplitMix64(seed=2) and unbiased Fisher-Yates per fragment. Original-position tags restore all atoms, including end markers, before lexical edits. It never consumes the generation RNG.


### Real selection and grouping witnesses

Input words `[0,0,3,0]`: Enjoy a warm meal and take a quiet pause.

| Stage | Actually observed |
| --- | --- |
| `03.catalogues` | 2 |
| `04.rejected` | 1 |
| `05.themes` | 4 |
| `06.rejected` | 3 |
| `07.structures` | 2 |
| `08.rejected` | 1 |
| `11.bank_rows` | 12 |
| `12.rejected` | 5 |
| `13.grouping` | 7 |
| `14.complete` | 2 |
| `15.paired_word` | 2 |
| `17.fragments` | 4 |
| `18.with_suffix` | 5 |
| `19.transition.paired` | 5 |
| `20.transition.unpaired` | 5 |
| `21.transition.singleton` | 5 |
| `22.transition.unwrapped` | 5 |
| `23.transition.shuffled` | 5 |
| `24.transition.restored` | 5 |
| `25.atoms` | 11 |
| `26.with_insertions` | 12 |
| `27.removed` | 1 |
| `28.stopped` | 1 |

Selection still pairs complete groups with first words and retries inside the same group. Fixed, chosen, suffix, and inserted atoms have distinct origins. Insertion retains the original anchor until the following filter actually removes it.

### Transition snapshots and real deletion

```text
18.with_suffix: Ok(Fragment { atoms: [Word("enjoy"), Word("a")], origin: Fixed { slot: 0 } })
19.transition.paired: (Ok(Fragment { atoms: [Word("enjoy"), Word("a")], origin: Fixed { slot: 0 } }), 0)
20.transition.unpaired: Ok(Fragment { atoms: [Word("enjoy"), Word("a")], origin: Fixed { slot: 0 } })
21.transition.singleton: Once { inner: IntoIter { inner: Item { opt: Some(Ok(Fragment { atoms: [Word("enjoy"), Word("a")], origin: Fixed { slot: 0 } })) } } }
22.transition.unwrapped: Ok(Fragment { atoms: [Word("enjoy"), Word("a")], origin: Fixed { slot: 0 } })
23.transition.shuffled: Ok(Shuffled { items: [(1, Word("a")), (0, Word("enjoy"))], origin: Fixed { slot: 0 }, seed: 2 })
24.transition.restored: Ok(Restored { atoms: [Word("enjoy"), Word("a")], origin: Fixed { slot: 0 } })
27.removed: Ok(Node { atom: Word("break"), origin: Suffix })
26.with_insertions: Ok(Node { atom: Word("pause"), origin: Inserted { rule: "body.meal", site: Suffix } })
```


## Unconstrained pool versus typed slots

For each frame, put both variable slots' alternatives into one pool and try its Cartesian square. Count compliance with the declared slot contracts, NOT grammaticality or naturalness.

| Locale | Untyped assignments | Contract-compatible | Typed assignments |
| --- | --- | --- | --- |
| zh-CN | 137 | 34 | 34 |
| en-US | 137 | 34 | 34 |

## Deterministic seed sweep

Seeds 0..10000. Counts are diagnostics, not probabilistic CI pass/fail thresholds.

| Locale | Distinct sentences | Frame counts (stable order) | Sentence count min/max | Choice draws/run (shuffle excluded) |
| --- | --- | --- | --- | --- |
| zh-CN | 34 | `[1206, 1209, 1254, 1263, 1264, 1316, 1241, 1247]` | 186 / 360 | `{4}` |
| en-US | 34 | `[1206, 1209, 1254, 1263, 1264, 1316, 1241, 1247]` | 186 / 360 | `{4}` |

All 20,000 revised outputs match the independent editorial oracle. Frame choices and draw counts still match the compact baseline; final text intentionally differs because actual words are inserted and deleted.


## All valid combinations (editorial review list)

### zh-CN

| Frame | Choices | Before edits | After edits |
| --- | --- | --- | --- |
| `body.meal` | `[0, 0]` | 好好吃饭，慢慢照顾自己。 | 好好吃饭，慢慢善待自己。 |
| `body.meal` | `[0, 1]` | 好好吃饭，用心照顾自己。 | 好好吃饭，用心善待自己。 |
| `body.meal` | `[1, 0]` | 记得吃饭，慢慢照顾自己。 | 记得吃饭，慢慢善待自己。 |
| `body.meal` | `[1, 1]` | 记得吃饭，用心照顾自己。 | 记得吃饭，用心善待自己。 |
| `body.meal` | `[2, 0]` | 安心吃饭，慢慢照顾自己。 | 安心吃饭，慢慢善待自己。 |
| `body.meal` | `[2, 1]` | 安心吃饭，用心照顾自己。 | 安心吃饭，用心善待自己。 |
| `body.water` | `[0, 0]` | 先喝点水，再轻轻伸展身体。 | 先喝点水，再轻轻舒展身体。 |
| `body.water` | `[0, 1]` | 先喝点水，再慢慢伸展身体。 | 先喝点水，再慢慢舒展身体。 |
| `body.water` | `[1, 0]` | 记得喝点水，再轻轻伸展身体。 | 记得喝点水，再轻轻舒展身体。 |
| `body.water` | `[1, 1]` | 记得喝点水，再慢慢伸展身体。 | 记得喝点水，再慢慢舒展身体。 |
| `rest.pause` | `[0, 0]` | 累了就歇一会儿，不必一直赶路。 | 累了就休息一会儿，不必一直赶路。 |
| `rest.pause` | `[0, 1]` | 累了就歇一会儿，不必一直硬撑。 | 累了就休息一会儿，不必一直硬撑。 |
| `rest.pause` | `[1, 0]` | 困了就歇一会儿，不必一直赶路。 | 困了就休息一会儿，不必一直赶路。 |
| `rest.pause` | `[1, 1]` | 困了就歇一会儿，不必一直硬撑。 | 困了就休息一会儿，不必一直硬撑。 |
| `rest.night` | `[0, 0]` | 今晚早点休息，明天再慢慢开始。 | 今晚早点休息，明天再慢慢出发。 |
| `rest.night` | `[0, 1]` | 今晚早点休息，明天再从容开始。 | 今晚早点休息，明天再从容出发。 |
| `rest.night` | `[1, 0]` | 今晚安心休息，明天再慢慢开始。 | 今晚安心休息，明天再慢慢出发。 |
| `rest.night` | `[1, 1]` | 今晚安心休息，明天再从容开始。 | 今晚安心休息，明天再从容出发。 |
| `step.begin` | `[0, 0]` | 先迈出一小步，按自己的节奏来。 | 先迈出一小步，按自己的步调来。 |
| `step.begin` | `[0, 1]` | 先迈出这一步，按自己的节奏来。 | 先迈出这一步，按自己的步调来。 |
| `step.begin` | `[1, 0]` | 先走出一小步，按自己的节奏来。 | 先走出一小步，按自己的步调来。 |
| `step.begin` | `[1, 1]` | 先走出这一步，按自己的节奏来。 | 先走出这一步，按自己的步调来。 |
| `step.credit` | `[0, 0]` | 今天有一点进展，就给自己一点肯定。 | 今天有一点进展，就送给自己一点肯定。 |
| `step.credit` | `[0, 1]` | 今天有一点进展，就给自己一点鼓励。 | 今天有一点进展，就送给自己一点鼓励。 |
| `step.credit` | `[1, 0]` | 今天有一点收获，就给自己一点肯定。 | 今天有一点收获，就送给自己一点肯定。 |
| `step.credit` | `[1, 1]` | 今天有一点收获，就给自己一点鼓励。 | 今天有一点收获，就送给自己一点鼓励。 |
| `accept.kindness` | `[0, 0]` | 暂时没做好，也值得被温柔对待。 | 这次没做好，也值得被温柔对待。 |
| `accept.kindness` | `[0, 1]` | 暂时没做好，也值得被认真对待。 | 这次没做好，也值得被认真对待。 |
| `accept.kindness` | `[1, 0]` | 暂时没做成，也值得被温柔对待。 | 这次没做成，也值得被温柔对待。 |
| `accept.kindness` | `[1, 1]` | 暂时没做成，也值得被认真对待。 | 这次没做成，也值得被认真对待。 |
| `accept.pace` | `[0, 0]` | 允许自己慢一点，不必急着证明什么。 | 允许自己慢一点，不必急着解释什么。 |
| `accept.pace` | `[0, 1]` | 允许自己慢一点，不必忙着证明什么。 | 允许自己慢一点，不必忙着解释什么。 |
| `accept.pace` | `[1, 0]` | 允许自己缓一缓，不必急着证明什么。 | 允许自己缓一缓，不必急着解释什么。 |
| `accept.pace` | `[1, 1]` | 允许自己缓一缓，不必忙着证明什么。 | 允许自己缓一缓，不必忙着解释什么。 |

### en-US

| Frame | Choices | Before edits | After edits |
| --- | --- | --- | --- |
| `body.meal` | `[0, 0]` | Enjoy a warm meal and take a quiet break. | Enjoy a warm meal and take a quiet pause. |
| `body.meal` | `[0, 1]` | Enjoy a warm meal and take a short break. | Enjoy a warm meal and take a short pause. |
| `body.meal` | `[1, 0]` | Enjoy a simple meal and take a quiet break. | Enjoy a simple meal and take a quiet pause. |
| `body.meal` | `[1, 1]` | Enjoy a simple meal and take a short break. | Enjoy a simple meal and take a short pause. |
| `body.meal` | `[2, 0]` | Enjoy a hearty meal and take a quiet break. | Enjoy a hearty meal and take a quiet pause. |
| `body.meal` | `[2, 1]` | Enjoy a hearty meal and take a short break. | Enjoy a hearty meal and take a short pause. |
| `body.water` | `[0, 0]` | Have some cool water and gently stretch your body. | Have some cool water, then gently stretch your body. |
| `body.water` | `[0, 1]` | Have some cool water and slowly stretch your body. | Have some cool water, then slowly stretch your body. |
| `body.water` | `[1, 0]` | Have some warm water and gently stretch your body. | Have some warm water, then gently stretch your body. |
| `body.water` | `[1, 1]` | Have some warm water and slowly stretch your body. | Have some warm water, then slowly stretch your body. |
| `rest.pause` | `[0, 0]` | Take a short break; you do not have to rush. | Take a short break; you do not need to rush. |
| `rest.pause` | `[0, 1]` | Take a short break; you do not have to hurry. | Take a short break; you do not need to hurry. |
| `rest.pause` | `[1, 0]` | Take a quiet break; you do not have to rush. | Take a quiet break; you do not need to rush. |
| `rest.pause` | `[1, 1]` | Take a quiet break; you do not have to hurry. | Take a quiet break; you do not need to hurry. |
| `rest.night` | `[0, 0]` | Get some good rest tonight and start tomorrow at a gentle pace. | Get some good rest tonight and approach tomorrow at a gentle pace. |
| `rest.night` | `[0, 1]` | Get some good rest tonight and start tomorrow at a steady pace. | Get some good rest tonight and approach tomorrow at a steady pace. |
| `rest.night` | `[1, 0]` | Get some quiet rest tonight and start tomorrow at a gentle pace. | Get some quiet rest tonight and approach tomorrow at a gentle pace. |
| `rest.night` | `[1, 1]` | Get some quiet rest tonight and start tomorrow at a steady pace. | Get some quiet rest tonight and approach tomorrow at a steady pace. |
| `step.begin` | `[0, 0]` | Take one small step at your own pace. | Try one small step at your own pace. |
| `step.begin` | `[0, 1]` | Take one small step to your own rhythm. | Try one small step to your own rhythm. |
| `step.begin` | `[1, 0]` | Take one manageable step at your own pace. | Try one manageable step at your own pace. |
| `step.begin` | `[1, 1]` | Take one manageable step to your own rhythm. | Try one manageable step to your own rhythm. |
| `step.credit` | `[0, 0]` | Give yourself credit for the small steps you take. | Give yourself credit for the small steps you have taken. |
| `step.credit` | `[0, 1]` | Give yourself credit for the steady steps you take. | Give yourself credit for the steady steps you have taken. |
| `step.credit` | `[1, 0]` | Give yourself recognition for the small steps you take. | Give yourself recognition for the small steps you have taken. |
| `step.credit` | `[1, 1]` | Give yourself recognition for the steady steps you take. | Give yourself recognition for the steady steps you have taken. |
| `accept.kindness` | `[0, 0]` | You deserve kindness, even when things feel difficult. | You deserve kindness, even when things seem difficult. |
| `accept.kindness` | `[0, 1]` | You deserve kindness, even when things feel uncertain. | You deserve kindness, even when things seem uncertain. |
| `accept.kindness` | `[1, 0]` | You deserve patience, even when things feel difficult. | You deserve patience, even when things seem difficult. |
| `accept.kindness` | `[1, 1]` | You deserve patience, even when things feel uncertain. | You deserve patience, even when things seem uncertain. |
| `accept.pace` | `[0, 0]` | Let yourself slow down; you do not need to rush. | Let yourself slow down; you do not have to rush. |
| `accept.pace` | `[0, 1]` | Let yourself slow down; you do not need to hurry. | Let yourself slow down; you do not have to hurry. |
| `accept.pace` | `[1, 0]` | Let yourself take a breath; you do not need to rush. | Let yourself take a breath; you do not have to rush. |
| `accept.pace` | `[1, 1]` | Let yourself take a breath; you do not need to hurry. | Let yourself take a breath; you do not have to hurry. |

## Actual revised lazy trace (seed=0, en-US)

Result: You deserve patience, even when things seem difficult.

Snapshots are observations of consumed values, NOT full intermediate collections.

| Stage | Observed | Kept | Omitted |
| --- | --- | --- | --- |
| `01.theme` | 1 | 1 | 0 |
| `02.structure` | 1 | 1 | 0 |
| `03.catalogues` | 2 | 2 | 0 |
| `04.kept` | 1 | 1 | 0 |
| `04.rejected` | 1 | 1 | 0 |
| `05.themes` | 4 | 2 | 2 |
| `06.kept` | 1 | 1 | 0 |
| `06.rejected` | 3 | 2 | 1 |
| `07.structures` | 2 | 2 | 0 |
| `08.kept` | 1 | 1 | 0 |
| `08.rejected` | 1 | 1 | 0 |
| `09.slots` | 2 | 2 | 0 |
| `10.slot_id` | 2 | 2 | 0 |
| `11.bank_rows` | 10 | 2 | 8 |
| `12.kept` | 6 | 2 | 4 |
| `12.rejected` | 4 | 2 | 2 |
| `13.grouping` | 6 | 2 | 4 |
| `14.complete` | 2 | 2 | 0 |
| `15.paired_word` | 2 | 2 | 0 |
| `16.choice` | 2 | 2 | 0 |
| `17.fragments` | 4 | 2 | 2 |
| `18.with_suffix` | 5 | 2 | 3 |
| `19.transition.paired` | 5 | 2 | 3 |
| `20.transition.unpaired` | 5 | 2 | 3 |
| `21.transition.singleton` | 5 | 2 | 3 |
| `22.transition.unwrapped` | 5 | 2 | 3 |
| `23.transition.shuffled` | 5 | 2 | 3 |
| `24.transition.restored` | 5 | 2 | 3 |
| `25.atoms` | 11 | 2 | 9 |
| `26.with_insertions` | 12 | 2 | 10 |
| `27.kept` | 11 | 2 | 9 |
| `27.removed` | 1 | 1 | 0 |
| `28.passed` | 10 | 2 | 8 |
| `28.stopped` | 1 | 1 | 0 |
| `29.position` | 10 | 2 | 8 |
| `30.localized` | 10 | 2 | 8 |
| `31.spacing` | 10 | 2 | 8 |
| `32.pieces` | 17 | 2 | 15 |
| `33.capitalized` | 17 | 2 | 15 |
| `34.accumulated` | 17 | 2 | 15 |

First 16 actual events (not grouped by stage):

```text
01.theme: draw=0 word=0xe220a8397b1dcdaf bound=4 -> Some(3)
02.structure: draw=1 word=0x6e789e6aa1b965f4 bound=2 -> Some(0)
03.catalogues: Ok(Catalogue { choice: Choice { theme: 3, structure: 0 }, language: Zh })
04.rejected: Ok(Catalogue { choice: Choice { theme: 3, structure: 0 }, language: Zh })
03.catalogues: Ok(Catalogue { choice: Choice { theme: 3, structure: 0 }, language: En })
04.kept: Ok(Catalogue { choice: Choice { theme: 3, structure: 0 }, language: En })
05.themes: Ok(ThemeRow { catalogue: Catalogue { choice: Choice { theme: 3, structure: 0 }, language: En }, theme: 0 })
06.rejected: Ok(ThemeRow { catalogue: Catalogue { choice: Choice { theme: 3, structure: 0 }, language: En }, theme: 0 })
05.themes: Ok(ThemeRow { catalogue: Catalogue { choice: Choice { theme: 3, structure: 0 }, language: En }, theme: 1 })
06.rejected: Ok(ThemeRow { catalogue: Catalogue { choice: Choice { theme: 3, structure: 0 }, language: En }, theme: 1 })
05.themes: Ok(ThemeRow { catalogue: Catalogue { choice: Choice { theme: 3, structure: 0 }, language: En }, theme: 2 })
06.rejected: Ok(ThemeRow { catalogue: Catalogue { choice: Choice { theme: 3, structure: 0 }, language: En }, theme: 2 })
05.themes: Ok(ThemeRow { catalogue: Catalogue { choice: Choice { theme: 3, structure: 0 }, language: En }, theme: 3 })
06.kept: Ok(ThemeRow { catalogue: Catalogue { choice: Choice { theme: 3, structure: 0 }, language: En }, theme: 3 })
07.structures: Ok(accept.kindness En index=6 choice=Choice { theme: 3, structure: 0 })
08.kept: Ok(accept.kindness En index=6 choice=Choice { theme: 3, structure: 0 })
```

Observed 220 events; kept 16, omitted 204.

## Limits of this evidence

- Naturalness requires editorial judgment; compatibility tests cannot certify it.
- The first meal slot has three alternatives; deterministic injected words exercise rejection in the real corpus and in a separate three-choice fixture. A 10,000-seed sweep is not expected to hit the very rare u64 rejection by chance.
- `fold(Result<...>)` preserves errors but keeps pulling; `try_fold` short-circuits. This bounded lab deliberately retains the requested fold and tests that distinction.
- No TUI, entropy-source/no-argument path, locale environment detection, installation, or cross-platform execution is claimed.
- The expanded chain has 34 executed operations: 28 content and 6 explicitly requested reversible transitions. This meets the revised total-stage range, NOT the old requirement of 32 effective content operations. No production CLI/TUI integration or frozen generation protocol is claimed.
