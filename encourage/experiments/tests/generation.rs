use encourage_experiments::corpus::{self, Atom, Frame, Mark, Slot};
use encourage_experiments::pipeline::{
    choice_vectors, generate, generate_words, reference, render_choices, run_frame, OPERATIONS,
};
use encourage_experiments::rng::{seed_from_argv, Budgeted};
use encourage_experiments::trace::{Trace, TraceConfig};
use encourage_experiments::{Error, Language, Limits};
use std::{cell::RefCell, collections::BTreeSet};

#[test]
fn exhaustive_corpus_has_34_distinct_sentences_per_language() {
    for language in Language::ALL {
        let mut distinct = BTreeSet::new();
        for (frame_index, frame) in corpus::frames(language).iter().enumerate() {
            corpus::validate(frame).unwrap();
            let combinations = choice_vectors(frame);
            assert_eq!(combinations.len(), if frame_index == 0 { 6 } else { 4 });
            assert_eq!(frame.id, corpus::frames(Language::En)[frame_index].id);
            for choices in combinations {
                let expected = render_choices(frame, &choices).unwrap();
                let raw = [
                    frame_index as u64 / 2,
                    frame_index as u64 % 2,
                    (choices[0] + frame.slots[0].variants.len()) as u64,
                    (choices[1] + frame.slots[1].variants.len()) as u64,
                ];
                let actual = generate_words(
                    raw.into_iter(),
                    language,
                    TraceConfig::default(),
                    Limits::default(),
                )
                .unwrap();
                assert_eq!(actual.sentence, expected);
                assert_eq!(actual.draws, 4);
                assert_eq!(actual.trace.stages["06.stopped"].observed, 1);
                assert!(!expected.contains("  "));
                assert!(
                    !expected.contains(" ,")
                        && !expected.contains(" ;")
                        && !expected.contains(" .")
                );
                assert!(expected.chars().all(|c| !c.is_control()));
                assert_eq!(expected.trim(), expected);
                assert!(expected.len() <= Limits::default().bytes);
                match language {
                    Language::Zh => {
                        assert!(expected.ends_with('。'));
                        assert!(!expected.contains(' '));
                        assert!((10..=30).contains(&expected.chars().count()));
                    }
                    Language::En => {
                        assert!(expected.ends_with('.'));
                        assert!(expected.as_bytes()[0].is_ascii_uppercase());
                        assert!((6..=18).contains(&expected.split_whitespace().count()));
                    }
                }
                assert!(
                    distinct.insert(expected),
                    "duplicate across alternatives or frames"
                );
            }
        }
        assert_eq!(distinct.len(), 34);
    }
}

#[test]
fn independent_oracle_and_bilingual_frame_match_across_10000_seeds() {
    let mut covered = [BTreeSet::new(), BTreeSet::new()];
    for seed in 0..10_000 {
        let zh = generate(seed, Language::Zh, TraceConfig::off()).unwrap();
        let en = generate(seed, Language::En, TraceConfig::off()).unwrap();
        assert_eq!(zh.frame_index, en.frame_index);
        for (i, language, actual) in [(0, Language::Zh, zh), (1, Language::En, en)] {
            let expected = reference(seed, language).unwrap();
            assert_eq!(
                (actual.frame_index, actual.sentence.clone(), actual.draws),
                expected
            );
            covered[i].insert(actual.sentence);
        }
    }
    assert_eq!(covered.map(|s| s.len()), [34, 34]);
}

#[test]
fn seed_zero_golden_outputs() {
    assert_eq!(
        generate(0, Language::Zh, TraceConfig::off())
            .unwrap()
            .sentence,
        "暂时没做成，也值得被温柔对待。"
    );
    assert_eq!(
        generate(0, Language::En, TraceConfig::off())
            .unwrap()
            .sentence,
        "You deserve patience, even when things feel difficult."
    );
}

#[test]
fn tracing_and_preview_caps_do_not_change_generation_or_consumption() {
    for language in Language::ALL {
        for seed in [0, 1, 42, u64::MAX, seed_from_argv(&["今天有点累"]).unwrap()] {
            let full = generate(seed, language, TraceConfig::default()).unwrap();
            let minimal = generate(
                seed,
                language,
                TraceConfig {
                    enabled: true,
                    previews: 0,
                    events: 0,
                },
            )
            .unwrap();
            let off = generate(seed, language, TraceConfig::off()).unwrap();
            assert_eq!(full.sentence, minimal.sentence);
            assert_eq!(full.sentence, off.sentence);
            assert_eq!(full.draws, minimal.draws);
            assert_eq!(full.draws, off.draws);
            assert!(off.trace.stages.is_empty());
            assert_eq!(full.trace.observed_events, minimal.trace.observed_events);
            for (name, snapshot) in &full.trace.stages {
                let capped = &minimal.trace.stages[name];
                assert_eq!(snapshot.observed, capped.observed);
                assert_eq!(capped.omitted(), capped.observed);
                assert!(snapshot.preview.len() <= 16);
            }
            assert!(full.trace.events.len() <= 128);
        }
    }
}

#[test]
fn actual_lazy_events_interleave_instead_of_running_whole_stages() {
    let actual = generate(
        0,
        Language::En,
        TraceConfig {
            enabled: true,
            previews: 2,
            events: 1024,
        },
    )
    .unwrap();
    let events = &actual.trace.events;
    let attempts: Vec<_> = events
        .iter()
        .enumerate()
        .filter(|(_, e)| e.stage == "source.attempt")
        .map(|(i, _)| i)
        .collect();
    let first_fold = events
        .iter()
        .position(|e| e.stage == "12.accumulated")
        .unwrap();
    assert_eq!(attempts.len(), 2);
    assert!(attempts[0] < first_fold && first_fold < attempts[1]);
    assert_eq!(
        actual.trace.stages["05.with_suffix"].observed,
        actual.trace.stages["06.passed"].observed + 1
    );
    assert_eq!(actual.trace.stages["06.stopped"].observed, 1);
}

#[test]
fn three_candidate_frame_rejects_before_zip_without_losing_a_slot() {
    use Atom::{End, Punctuation as P, Word as W};
    let frame = Frame {
        id: "test.three",
        language: Language::En,
        slots: &[
            Slot {
                before: &[W("a")],
                variants: &[&[W("small")], &[W("kind")], &[W("gentle")]],
            },
            Slot {
                before: &[],
                variants: &[&[W("pause")], &[W("break")]],
            },
        ],
        suffix: &[P(Mark::Period), End],
    };
    let mut rng = Budgeted::new([0, 1, 0, 99].into_iter(), 4);
    let trace = RefCell::new(Trace::new(TraceConfig::default()));
    assert_eq!(
        run_frame(&frame, &mut rng, &trace, Limits::default()).unwrap(),
        "A kind pause."
    );
    assert_eq!(rng.used, 3);
    assert_eq!(rng.word(), Ok(99));
    let trace = trace.into_inner();
    assert_eq!(trace.stages["01.rejected"].observed, 1);
    assert_eq!(trace.stages["02.paired"].observed, 2);
}

#[test]
fn real_corpus_rejection_retries_the_same_slot() {
    let actual = generate_words(
        [0, 0, 0, 3, 1].into_iter(),
        Language::En,
        TraceConfig::default(),
        Limits::default(),
    )
    .unwrap();
    assert_eq!(actual.frame_index, 0);
    assert_eq!(actual.sentence, "Enjoy a warm meal and take a short break.");
    assert_eq!(actual.draws, 5);
    assert_eq!(actual.trace.stages["01.rejected"].observed, 1);
    assert_eq!(actual.trace.stages["02.paired"].observed, 2);
}

#[test]
fn resource_limits_and_exhaustion_fail_without_a_sentence() {
    for (limits, expected) in [
        (
            Limits {
                draws: 3,
                ..Limits::default()
            },
            Error::DrawBudget,
        ),
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
        assert_eq!(
            generate_words(
                std::iter::repeat(1),
                Language::En,
                TraceConfig::off(),
                limits
            )
            .unwrap_err(),
            expected
        );
    }
    assert_eq!(
        generate_words(
            [0, 0, 0].into_iter(),
            Language::En,
            TraceConfig::off(),
            Limits::default()
        )
        .unwrap_err(),
        Error::RandomSourceEnded
    );
}

#[test]
fn invalid_candidates_and_unsafe_atoms_are_rejected() {
    use Atom::{End, Punctuation as P, Word as W};
    // Scoped frames borrow promoted static arrays; test invalid source data,
    // never recover by inserting a stock sentence.
    let empty = Frame {
        id: "bad",
        language: Language::En,
        slots: &[
            Slot {
                before: &[],
                variants: &[],
            },
            Slot {
                before: &[],
                variants: &[&[W("a")], &[W("b")]],
            },
        ],
        suffix: &[P(Mark::Period), End],
    };
    assert_eq!(corpus::validate(&empty), Err(Error::EmptyCandidates));
    let duplicate = Frame {
        id: "bad",
        language: Language::En,
        slots: &[
            Slot {
                before: &[],
                variants: &[&[W("a")], &[W("a")]],
            },
            Slot {
                before: &[],
                variants: &[&[W("c")], &[W("d")]],
            },
        ],
        suffix: &[P(Mark::Period), End],
    };
    assert!(corpus::validate(&duplicate).is_err());
    let unsafe_words = Frame {
        id: "bad",
        language: Language::En,
        slots: &[
            Slot {
                before: &[],
                variants: &[&[W("hello world")], &[W("\x1b")]],
            },
            Slot {
                before: &[],
                variants: &[&[W("c")], &[W("d")]],
            },
        ],
        suffix: &[P(Mark::Period), End],
    };
    assert!(corpus::validate(&unsafe_words).is_err());
}

#[test]
fn operation_count_is_honest_and_markers_match_the_compiled_source() {
    let source = include_str!("../src/pipeline.rs");
    let display = source
        .split("// encourage:display:start")
        .nth(1)
        .unwrap()
        .split("// encourage:display:end")
        .next()
        .unwrap();
    assert_eq!(OPERATIONS.len(), 12);
    assert!(
        !(32..=36).contains(&OPERATIONS.len()),
        "do not claim the P0 long-chain gate"
    );
    assert_eq!(display.matches("// op").count(), OPERATIONS.len());
    for (i, (method, _)) in OPERATIONS.iter().enumerate() {
        let marker = format!("// op{:02}: {method}", i + 1);
        assert_eq!(display.matches(&marker).count(), 1);
        let tail = display.split(&marker).nth(1).unwrap().trim_start();
        assert!(tail.starts_with(&format!(".{method}(")));
    }
}
