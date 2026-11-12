//! Reproducible research report, not a user-facing CLI or a new flag surface.
use encourage_experiments::pipeline::{choice_vectors, generate, render_choices, OPERATIONS};
use encourage_experiments::rng::{seed_from_argv, SplitMix64};
use encourage_experiments::trace::TraceConfig;
use encourage_experiments::Language;
use encourage_experiments::{corpus, expanded, revision};
use std::collections::{BTreeMap, BTreeSet};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("# Encourage P0 reproducible observations\n");
    println!("Experimental corpus and consumption order; NOT a frozen encourage-gen-v1 release.\n");
    println!("## Seed mapping\n\n| argv | seed (hex) |\n| --- | --- |");
    for args in [
        vec![""],
        vec!["hello"],
        vec!["今天有点累"],
        vec!["a", "b"],
        vec!["a b"],
        vec!["42"],
    ] {
        println!("| `{args:?}` | `{:016x}` |", seed_from_argv(&args)?);
    }
    println!(
        "\nSplitMix64(seed=0): `{:016x?}`\n",
        SplitMix64::new(0).take(5).collect::<Vec<_>>()
    );

    println!("## Compact chain\n\n{} actual main-chain operations; target 32–36 NOT met. Header choices and nested helpers are excluded.\n", OPERATIONS.len());
    println!("| # | Adapter | Purpose |\n| --- | --- | --- |");
    for (i, (name, purpose)) in OPERATIONS.iter().enumerate() {
        println!("| {} | `{name}` | {purpose} |", i + 1);
    }

    println!("\n## Expanded chain: explicit transitions then real edits\n\n{} executed operations = {} content + {} reversible transitions. The new requested transition policy permits these no-ops; they are NOT described as content transformations. Nested helpers and inspect remain excluded.\n", expanded::OPERATIONS.len(), expanded::CONTENT_OPERATION_COUNT, expanded::TRANSITION_STAGES.len());
    println!("| # | Kind | Adapter | Purpose |\n| --- | --- | --- | --- |");
    for (i, (name, purpose)) in expanded::OPERATIONS.iter().enumerate() {
        let kind = if expanded::TRANSITION_STAGES.contains(&(i + 1)) {
            "transition"
        } else {
            "content"
        };
        println!("| {} | {kind} | `{name}` | {purpose} |", i + 1);
    }
    println!("\nShuffle uses independent SplitMix64(seed={}) and unbiased Fisher-Yates per fragment. Original-position tags restore all atoms, including end markers, before lexical edits. It never consumes the generation RNG.\n", expanded::SHUFFLE_SEED);
    let witness = expanded::generate_words(
        [0, 0, 3, 0].into_iter(),
        Language::En,
        TraceConfig::default(),
        encourage_experiments::Limits::default(),
    )?;
    println!(
        "\n### Real selection and grouping witnesses\n\nInput words `[0,0,3,0]`: {}\n",
        witness.sentence
    );
    println!("| Stage | Actually observed |\n| --- | --- |");
    for stage in [
        "03.catalogues",
        "04.rejected",
        "05.themes",
        "06.rejected",
        "07.structures",
        "08.rejected",
        "11.bank_rows",
        "12.rejected",
        "13.grouping",
        "14.complete",
        "15.paired_word",
        "17.fragments",
        "18.with_suffix",
        "19.transition.paired",
        "20.transition.unpaired",
        "21.transition.singleton",
        "22.transition.unwrapped",
        "23.transition.shuffled",
        "24.transition.restored",
        "25.atoms",
        "26.with_insertions",
        "27.removed",
        "28.stopped",
    ] {
        println!("| `{stage}` | {} |", witness.trace.stages[stage].observed);
    }
    println!("\nSelection still pairs complete groups with first words and retries inside the same group. Fixed, chosen, suffix, and inserted atoms have distinct origins. Insertion retains the original anchor until the following filter actually removes it.\n");
    println!("### Transition snapshots and real deletion\n\n```text");
    for stage in [
        "18.with_suffix",
        "19.transition.paired",
        "20.transition.unpaired",
        "21.transition.singleton",
        "22.transition.unwrapped",
        "23.transition.shuffled",
        "24.transition.restored",
        "27.removed",
    ] {
        println!("{stage}: {}", witness.trace.stages[stage].preview[0]);
    }
    for value in &witness.trace.stages["26.with_insertions"].preview {
        if value.contains("Inserted") {
            println!("26.with_insertions: {value}");
        }
    }
    println!("```\n");

    println!("\n## Unconstrained pool versus typed slots\n");
    println!("For each frame, put both variable slots' alternatives into one pool and try its Cartesian square. Count compliance with the declared slot contracts, NOT grammaticality or naturalness.\n");
    println!("| Locale | Untyped assignments | Contract-compatible | Typed assignments |\n| --- | --- | --- | --- |");
    for language in Language::ALL {
        let mut untyped = 0;
        let mut compatible = 0;
        let mut typed = 0;
        for frame in corpus::frames(language) {
            let pool: BTreeSet<_> = frame
                .slots
                .iter()
                .flat_map(|slot| slot.variants.iter().copied())
                .collect();
            for a in &pool {
                for b in &pool {
                    untyped += 1;
                    compatible += usize::from(
                        frame.slots[0].variants.contains(a) && frame.slots[1].variants.contains(b),
                    );
                }
            }
            typed += choice_vectors(frame).len();
        }
        println!(
            "| {} | {untyped} | {compatible} | {typed} |",
            language.name()
        );
    }

    println!("\n## Deterministic seed sweep\n\nSeeds 0..10000. Counts are diagnostics, not probabilistic CI pass/fail thresholds.\n");
    println!("| Locale | Distinct sentences | Frame counts (stable order) | Sentence count min/max | Choice draws/run (shuffle excluded) |\n| --- | --- | --- | --- | --- |");
    for language in Language::ALL {
        let mut sentences = BTreeMap::new();
        let mut frames = [0_usize; 8];
        let mut draws = BTreeSet::new();
        for seed in 0..10_000 {
            let generated = generate(seed, language, TraceConfig::off())?;
            let extended = expanded::generate(seed, language, TraceConfig::off())?;
            assert_eq!(generated.frame_index, extended.frame_index);
            assert_ne!(generated.sentence, extended.sentence);
            assert_eq!(generated.draws, extended.draws);
            let oracle = revision::reference(seed, language)?;
            assert_eq!(
                (
                    extended.frame_index,
                    extended.sentence.clone(),
                    extended.draws
                ),
                oracle
            );
            *sentences.entry(extended.sentence).or_insert(0_usize) += 1;
            frames[extended.frame_index] += 1;
            draws.insert(extended.draws);
        }
        println!(
            "| {} | {} | `{frames:?}` | {} / {} | `{draws:?}` |",
            language.name(),
            sentences.len(),
            sentences.values().min().unwrap(),
            sentences.values().max().unwrap()
        );
    }

    println!("\nAll 20,000 revised outputs match the independent editorial oracle. Frame choices and draw counts still match the compact baseline; final text intentionally differs because actual words are inserted and deleted.\n");
    println!("\n## All valid combinations (editorial review list)\n");
    for language in Language::ALL {
        println!("### {}\n", language.name());
        println!("| Frame | Choices | Before edits | After edits |\n| --- | --- | --- | --- |");
        for (index, frame) in corpus::frames(language).iter().enumerate() {
            for choices in choice_vectors(frame) {
                let actual = expanded::generate_words(
                    [
                        index as u64 / 2,
                        index as u64 % 2,
                        (choices[0] + frame.slots[0].variants.len()) as u64,
                        (choices[1] + frame.slots[1].variants.len()) as u64,
                    ]
                    .into_iter(),
                    language,
                    TraceConfig::off(),
                    encourage_experiments::Limits::default(),
                )?;
                assert_eq!(
                    actual.sentence,
                    revision::render_choices(index, language, &choices)?
                );
                println!(
                    "| `{}` | `{choices:?}` | {} | {} |",
                    frame.id,
                    render_choices(frame, &choices)?,
                    actual.sentence
                );
            }
        }
        println!();
    }

    println!("## Actual revised lazy trace (seed=0, en-US)\n");
    let generated = expanded::generate(
        0,
        Language::En,
        TraceConfig {
            enabled: true,
            previews: 2,
            events: 16,
        },
    )?;
    println!("Result: {}\n", generated.sentence);
    println!("Snapshots are observations of consumed values, NOT full intermediate collections.\n");
    println!("| Stage | Observed | Kept | Omitted |\n| --- | --- | --- | --- |");
    for (name, snapshot) in &generated.trace.stages {
        println!(
            "| `{name}` | {} | {} | {} |",
            snapshot.observed,
            snapshot.preview.len(),
            snapshot.omitted()
        );
    }
    println!("\nFirst 16 actual events (not grouped by stage):\n\n```text");
    for event in &generated.trace.events {
        println!("{}: {}", event.stage, event.value);
    }
    println!(
        "```\n\nObserved {} events; kept {}, omitted {}.\n",
        generated.trace.observed_events,
        generated.trace.events.len(),
        generated.trace.observed_events - generated.trace.events.len()
    );
    println!("## Limits of this evidence\n\n- Naturalness requires editorial judgment; compatibility tests cannot certify it.\n- The first meal slot has three alternatives; deterministic injected words exercise rejection in the real corpus and in a separate three-choice fixture. A 10,000-seed sweep is not expected to hit the very rare u64 rejection by chance.\n- `fold(Result<...>)` preserves errors but keeps pulling; `try_fold` short-circuits. This bounded lab deliberately retains the requested fold and tests that distinction.\n- No TUI, entropy-source/no-argument path, locale environment detection, installation, or cross-platform execution is claimed.\n- The expanded chain has 34 executed operations: 28 content and 6 explicitly requested reversible transitions. This meets the revised total-stage range, NOT the old requirement of 32 effective content operations. No production CLI/TUI integration or frozen generation protocol is claimed.");
    Ok(())
}
