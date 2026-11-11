use encourage_experiments::expanded::{
    self, CONTENT_OPERATION_COUNT, OPERATIONS, TRANSITION_STAGES,
};
use encourage_experiments::pipeline::{self, choice_vectors};
use encourage_experiments::trace::TraceConfig;
use encourage_experiments::{corpus, revision};
use encourage_experiments::{Error, Language, Limits};
use std::collections::BTreeSet;

#[test]
fn revised_output_matches_oracle_and_preserves_draws_across_10000_seeds() {
    let mut covered = [BTreeSet::new(), BTreeSet::new()];
    for seed in 0..10_000 {
        let mut bilingual_frame = None;
        for (index, language) in Language::ALL.into_iter().enumerate() {
            let expanded = expanded::generate(seed, language, TraceConfig::off()).unwrap();
            let compact = pipeline::generate(seed, language, TraceConfig::off()).unwrap();
            let reference = revision::reference(seed, language).unwrap();
            assert_eq!(
                (
                    expanded.frame_index,
                    expanded.sentence.clone(),
                    expanded.draws
                ),
                reference
            );
            assert_ne!(expanded.sentence, compact.sentence);
            assert_eq!(expanded.frame_index, compact.frame_index);
            assert_eq!(expanded.draws, compact.draws);
            if let Some(frame) = bilingual_frame {
                assert_eq!(expanded.frame_index, frame);
            }
            bilingual_frame = Some(expanded.frame_index);
            covered[index].insert(expanded.sentence);
        }
    }
    assert_eq!(covered.map(|s| s.len()), [34, 34]);
}

#[test]
fn every_corpus_combination_is_reachable_without_an_extra_draw() {
    for language in Language::ALL {
        for (frame_index, frame) in corpus::frames(language).iter().enumerate() {
            for choices in choice_vectors(frame) {
                let mut source = [
                    frame_index as u64 / 2,
                    frame_index as u64 % 2,
                    (choices[0] + frame.slots[0].variants.len()) as u64,
                    (choices[1] + frame.slots[1].variants.len()) as u64,
                    999,
                ]
                .into_iter();
                let output = expanded::generate_words(
                    source.by_ref(),
                    language,
                    TraceConfig::off(),
                    Limits::default(),
                )
                .unwrap();
                assert_eq!(
                    output.sentence,
                    revision::render_choices(frame_index, language, &choices).unwrap()
                );
                assert_eq!(output.draws, 4);
                assert_eq!(source.next(), Some(999));
            }
        }
    }
}

#[test]
fn catalogue_and_role_filters_have_real_rejected_inputs() {
    let output = expanded::generate_words(
        [0, 0, 3, 0].into_iter(),
        Language::En,
        TraceConfig::default(),
        Limits::default(),
    )
    .unwrap();
    let stages = &output.trace.stages;
    assert_eq!(stages["03.catalogues"].observed, 2);
    assert_eq!(stages["04.rejected"].observed, 1);
    assert_eq!(stages["05.themes"].observed, 4);
    assert_eq!(stages["06.rejected"].observed, 3);
    assert_eq!(stages["07.structures"].observed, 2);
    assert_eq!(stages["08.rejected"].observed, 1);
    // Five real alternatives queried for each of two slots, plus two boundaries.
    assert_eq!(stages["11.bank_rows"].observed, 12);
    assert_eq!(stages["12.rejected"].observed, 5);
    assert_eq!(stages["12.kept"].observed, 7);
    assert_eq!(stages["13.grouping"].observed, 7);
    assert_eq!(stages["14.complete"].observed, 2);
    assert_eq!(stages["15.paired_word"].observed, 2);
    assert_eq!(stages["17.fragments"].observed, 4);
    assert_eq!(stages["18.with_suffix"].observed, 5);
    assert_eq!(stages["28.stopped"].observed, 1);
    assert_eq!(stages["27.removed"].observed, 1);
    assert_eq!(stages["27.kept"].observed, stages["28.passed"].observed + 1);
    assert_eq!(
        stages["26.with_insertions"].observed,
        stages["25.atoms"].observed + 1
    );
}

#[test]
fn local_retries_do_not_advance_zip_or_consume_the_next_slot() {
    let script = [0, 0, 0, 0, 3, 1, 999];
    let mut source = script.into_iter();
    let output = expanded::generate_words(
        source.by_ref(),
        Language::En,
        TraceConfig::default(),
        Limits::default(),
    )
    .unwrap();
    assert_eq!(output.sentence, "Enjoy a warm meal and take a short pause.");
    assert_eq!(output.draws, 6);
    assert_eq!(source.next(), Some(999));
    assert_eq!(output.trace.stages["random.rejected"].observed, 2);
    assert_eq!(output.trace.stages["15.paired_word"].observed, 2);
    assert_eq!(output.trace.stages["16.choice"].observed, 4);
    let compact = pipeline::generate_words(
        script.into_iter(),
        Language::En,
        TraceConfig::off(),
        Limits::default(),
    )
    .unwrap();
    assert_ne!(output.sentence, compact.sentence);
    assert_eq!(output.draws, compact.draws);
}

#[test]
fn the_longer_chain_still_executes_lazily_per_item() {
    let output = expanded::generate(
        0,
        Language::En,
        TraceConfig {
            enabled: true,
            previews: 2,
            events: 1024,
        },
    )
    .unwrap();
    let events = &output.trace.events;
    let paired: Vec<_> = events
        .iter()
        .enumerate()
        .filter(|(_, e)| e.stage == "15.paired_word")
        .map(|(i, _)| i)
        .collect();
    let first_fold = events
        .iter()
        .position(|e| e.stage == "34.accumulated")
        .unwrap();
    assert_eq!(paired.len(), 2);
    assert!(paired[0] < first_fold && first_fold < paired[1]);
}

#[test]
fn fixed_and_random_fragments_keep_distinct_provenance() {
    let output = expanded::generate_words(
        [0, 0, 3, 0].into_iter(),
        Language::En,
        TraceConfig {
            enabled: true,
            previews: 64,
            events: 0,
        },
        Limits::default(),
    )
    .unwrap();
    let atoms = &output.trace.stages["25.atoms"].preview;
    assert!(atoms
        .iter()
        .any(|s| s.contains("Word(\"enjoy\")") && s.contains("Fixed { slot: 0 }")));
    assert!(atoms
        .iter()
        .any(|s| s.contains("Word(\"warm\")") && s.contains("Choice { slot: 0, draw: 2 }")));
    assert!(atoms
        .iter()
        .any(|s| s.contains("Word(\"break\")") && s.contains("Suffix")));
    assert!(atoms
        .iter()
        .all(|s| !s.contains("Word(\"enjoy\")") || !s.contains("Choice")));
}

#[test]
fn trace_caps_and_disabled_tracing_preserve_results_and_counts() {
    for language in Language::ALL {
        for seed in [0, 1, 42, u64::MAX] {
            let full = expanded::generate(seed, language, TraceConfig::default()).unwrap();
            let counts_only = expanded::generate(
                seed,
                language,
                TraceConfig {
                    enabled: true,
                    previews: 0,
                    events: 0,
                },
            )
            .unwrap();
            let off = expanded::generate(seed, language, TraceConfig::off()).unwrap();
            assert_eq!(full.sentence, counts_only.sentence);
            assert_eq!(full.sentence, off.sentence);
            assert_eq!(full.draws, counts_only.draws);
            assert_eq!(full.draws, off.draws);
            assert!(off.trace.stages.is_empty());
            assert_eq!(
                full.trace.observed_events,
                counts_only.trace.observed_events
            );
            for (stage, snapshot) in &full.trace.stages {
                let other = &counts_only.trace.stages[stage];
                assert_eq!(snapshot.observed, other.observed);
                assert_eq!(other.omitted(), other.observed);
                assert!(snapshot.preview.len() <= 16);
            }
            assert!(full.trace.events.len() <= 128);
        }
    }
}

#[test]
fn errors_are_not_filtered_out_or_replaced_by_suffix_text() {
    for draws in 0..4 {
        let result = expanded::generate_words(
            std::iter::repeat(1),
            Language::En,
            TraceConfig::off(),
            Limits {
                draws,
                ..Limits::default()
            },
        );
        assert_eq!(result.unwrap_err(), Error::DrawBudget);
    }
    for (limits, expected) in [
        (
            Limits {
                atoms: 1,
                ..Limits::default()
            },
            Error::AtomBudget,
        ),
        (
            Limits {
                bytes: 1,
                ..Limits::default()
            },
            Error::ByteBudget,
        ),
    ] {
        let result = expanded::generate_words(
            std::iter::repeat(1),
            Language::En,
            TraceConfig::off(),
            limits,
        );
        assert_eq!(result.unwrap_err(), expected);
    }
    for script in [vec![], vec![0], vec![0, 0], vec![0, 0, 0]] {
        let result = expanded::generate_words(
            script.into_iter(),
            Language::En,
            TraceConfig::off(),
            Limits::default(),
        );
        assert_eq!(result.unwrap_err(), Error::RandomSourceEnded);
    }
    // Repeated rejected words are bounded rather than silently modulo-selected.
    let result = expanded::generate_words(
        std::iter::repeat(0),
        Language::En,
        TraceConfig::off(),
        Limits {
            draws: 8,
            ..Limits::default()
        },
    );
    assert_eq!(result.unwrap_err(), Error::DrawBudget);
}

#[test]
fn thirty_four_markers_separate_content_and_transition_operations() {
    let source = include_str!("../src/expanded.rs");
    let display = source
        .split("// encourage:display:start")
        .nth(1)
        .unwrap()
        .split("// encourage:display:end")
        .next()
        .unwrap();
    assert_eq!(OPERATIONS.len(), 34);
    assert_eq!(display.matches("// op").count(), 34);
    assert!((32..=36).contains(&OPERATIONS.len()));
    assert_eq!(CONTENT_OPERATION_COUNT, 28);
    assert_eq!(TRANSITION_STAGES, [19, 20, 21, 22, 23, 24]);
    assert_eq!(display.matches(".fold(").count(), 1);
    assert!(!display
        .lines()
        .any(|line| line.starts_with("        .collect(")));
    for (index, (method, _)) in OPERATIONS.iter().enumerate() {
        let marker = format!("// op{:02}: {method}", index + 1);
        assert_eq!(display.matches(&marker).count(), 1);
        let after_marker = display.split(&marker).nth(1).unwrap();
        let (comment_tail, code) = after_marker.split_once('\n').unwrap();
        assert_eq!(
            comment_tail.contains("TRANSITION"),
            TRANSITION_STAGES.contains(&(index + 1))
        );
        assert_eq!(
            OPERATIONS[index].1.starts_with("TRANSITION"),
            TRANSITION_STAGES.contains(&(index + 1))
        );
        assert!(code.trim_start().starts_with(&format!(".{method}(")));
    }
}

#[test]
fn reversible_transition_snapshots_preserve_fragments_and_rng() {
    for language in Language::ALL {
        for seed in 0..32 {
            let output = expanded::generate(
                seed,
                language,
                TraceConfig {
                    enabled: true,
                    previews: 64,
                    events: 0,
                },
            )
            .unwrap();
            let stages = &output.trace.stages;
            assert_eq!(stages["18.with_suffix"], stages["20.transition.unpaired"]);
            assert_eq!(stages["18.with_suffix"], stages["22.transition.unwrapped"]);
            assert_eq!(
                stages["23.transition.shuffled"].observed,
                stages["24.transition.restored"].observed
            );
            assert!(stages["23.transition.shuffled"]
                .preview
                .iter()
                .all(|s| s.contains("seed: 2")));
            assert_eq!(
                output.draws,
                pipeline::generate(seed, language, TraceConfig::off())
                    .unwrap()
                    .draws
            );
        }
    }
}

#[test]
fn all_revisions_really_insert_new_words_then_remove_one_original_word() {
    for language in Language::ALL {
        for (index, frame) in corpus::frames(language).iter().enumerate() {
            let rule = revision::rule(language, index);
            revision::validate(frame, index).unwrap();
            for choices in choice_vectors(frame) {
                let output = expanded::generate_words(
                    [
                        index as u64 / 2,
                        index as u64 % 2,
                        (choices[0] + frame.slots[0].variants.len()) as u64,
                        (choices[1] + frame.slots[1].variants.len()) as u64,
                    ]
                    .into_iter(),
                    language,
                    TraceConfig {
                        enabled: true,
                        previews: 128,
                        events: 0,
                    },
                    Limits::default(),
                )
                .unwrap();
                let stages = &output.trace.stages;
                assert_eq!(
                    stages["26.with_insertions"].observed,
                    stages["25.atoms"].observed + rule.insert.len()
                );
                assert_eq!(stages["27.removed"].observed, 1);
                assert_eq!(
                    stages["27.kept"].observed + 1,
                    stages["26.with_insertions"].observed
                );
                assert!(stages["27.removed"].preview[0].contains(rule.remove));
                assert!(!stages["27.removed"].preview[0].contains("Inserted"));
                assert_eq!(
                    stages["27.kept"]
                        .preview
                        .iter()
                        .filter(|s| s.contains("Inserted"))
                        .count(),
                    rule.insert.len()
                );
                assert_ne!(
                    output.sentence,
                    pipeline::render_choices(frame, &choices).unwrap()
                );
                assert!(!output.sentence.contains("  ") && !output.sentence.contains(" ,"));
            }
        }
    }
}

#[test]
fn inserted_words_count_towards_final_limits_but_shuffle_does_not_use_choice_budget() {
    let script = [0, 1, 1, 0]; // warm water, gently
    let original = pipeline::generate_words(
        script.into_iter(),
        Language::En,
        TraceConfig::default(),
        Limits::default(),
    )
    .unwrap();
    let revised = expanded::generate_words(
        script.into_iter(),
        Language::En,
        TraceConfig::off(),
        Limits {
            draws: 4,
            ..Limits::default()
        },
    )
    .unwrap();
    assert_eq!(
        revised.sentence,
        "Have some warm water, then gently stretch your body."
    );
    assert_eq!(revised.draws, 4);
    for (limits, expected) in [
        (
            Limits {
                bytes: original.sentence.len(),
                ..Limits::default()
            },
            Error::ByteBudget,
        ),
        (
            Limits {
                atoms: original.trace.stages["06.passed"].observed,
                ..Limits::default()
            },
            Error::AtomBudget,
        ),
    ] {
        assert_eq!(
            expanded::generate_words(script.into_iter(), Language::En, TraceConfig::off(), limits)
                .unwrap_err(),
            expected
        );
    }
}

#[test]
fn expanded_trace_is_deterministic() {
    for language in Language::ALL {
        let a = expanded::generate(42, language, TraceConfig::default()).unwrap();
        let b = expanded::generate(42, language, TraceConfig::default()).unwrap();
        assert_eq!(a.trace.stages, b.trace.stages);
        assert_eq!(a.trace.events, b.trace.events);
        assert_eq!(a.trace.observed_events, b.trace.observed_events);
    }
}
