use super::*;
use std::collections::BTreeSet;

#[test]
fn every_combination_is_reachable_and_revisions_really_insert_then_delete() {
    for language in [Language::Chinese, Language::English] {
        let mut sentences = BTreeSet::new();
        for (index, frame) in corpus::frames(language).iter().enumerate() {
            corpus::validate(frame).unwrap();
            revision::validate(frame, index).unwrap();
            for a in 0..frame.slots[0].variants.len() {
                for b in 0..frame.slots[1].variants.len() {
                    let mut words = [
                        index as u64 / 2,
                        index as u64 % 2,
                        (a + frame.slots[0].variants.len()) as u64,
                        (b + frame.slots[1].variants.len()) as u64,
                        999,
                    ]
                    .into_iter();
                    let output = generate_words(
                        words.by_ref(),
                        language,
                        TraceConfig {
                            enabled: true,
                            previews: 128,
                            events: 0,
                        },
                        Limits::default(),
                    )
                    .unwrap();
                    assert_eq!(words.next(), Some(999));
                    assert_eq!(output.draws, 4);
                    assert_eq!(
                        output.sentence,
                        revision::render_choices(index, language, &[a, b]).unwrap()
                    );
                    assert!(sentences.insert(output.sentence.clone()));
                    assert!(!output.sentence.contains("  ") && !output.sentence.contains(" ,"));
                    assert!(!output.sentence.chars().any(char::is_control));
                    let stages = &output.trace.stages;
                    let rule = revision::rule(language, index);
                    assert_eq!(
                        stages["26.with_insertions"].observed,
                        stages["25.atoms"].observed + rule.insert.len()
                    );
                    assert_eq!(stages["27.removed"].observed, 1);
                    assert!(stages["27.removed"].preview[0].contains(rule.remove));
                    assert!(!stages["27.removed"].preview[0].contains("Inserted"));
                    assert_eq!(
                        stages["27.kept"]
                            .preview
                            .iter()
                            .filter(|value| value.contains("Inserted"))
                            .count(),
                        rule.insert.len()
                    );
                    assert_eq!(stages["28.stopped"].observed, 1);
                }
            }
        }
        assert_eq!(sentences.len(), 34);
    }
}

#[test]
fn local_rejection_does_not_consume_the_next_slot_or_an_extra_word() {
    let mut words = [0, 0, 0, 0, 3, 1, 999].into_iter();
    let output = generate_words(
        words.by_ref(),
        Language::English,
        TraceConfig::default(),
        Limits::default(),
    )
    .unwrap();
    assert_eq!(output.sentence, "Enjoy a warm meal and take a short pause.");
    assert_eq!(output.draws, 6);
    assert_eq!(words.next(), Some(999));
    assert_eq!(output.trace.stages["random.rejected"].observed, 2);
    assert_eq!(output.trace.stages["15.paired_word"].observed, 2);
    assert_eq!(output.trace.stages["16.choice"].observed, 4);
    assert_eq!(output.trace.stages["12.rejected"].observed, 5);
}

#[test]
fn errors_and_post_revision_budgets_never_produce_a_fallback_sentence() {
    for draws in 0..4 {
        assert_eq!(
            generate_words(
                std::iter::repeat(1),
                Language::English,
                TraceConfig::off(),
                Limits {
                    draws,
                    ..Limits::default()
                }
            )
            .unwrap_err(),
            GenerationError::DrawBudget
        );
    }
    for script in [vec![], vec![0], vec![0, 0], vec![0, 0, 0]] {
        assert_eq!(
            generate_words(
                script.into_iter(),
                Language::English,
                TraceConfig::off(),
                Limits::default()
            )
            .unwrap_err(),
            GenerationError::RandomSourceEnded
        );
    }
    assert_eq!(
        generate_words(
            std::iter::repeat(0),
            Language::English,
            TraceConfig::off(),
            Limits {
                draws: 8,
                ..Limits::default()
            }
        )
        .unwrap_err(),
        GenerationError::DrawBudget
    );
    let script = [0, 1, 1, 0];
    let full = generate_words(
        script.into_iter(),
        Language::English,
        TraceConfig::default(),
        Limits {
            draws: 4,
            ..Limits::default()
        },
    )
    .unwrap();
    assert_eq!(
        full.sentence,
        "Have some warm water, then gently stretch your body."
    );
    for (stage, expected) in [
        (16, "warm gently"),
        (17, "have some warm water and gently"),
        (18, "have some warm water and gently stretch your body."),
        (
            26,
            "have some warm water, then and gently stretch your body.",
        ),
        (27, "have some warm water, then gently stretch your body."),
        (31, "have some warm water, then gently stretch your body."),
        (33, "Have some warm water, then gently stretch your body."),
        (34, "Have some warm water, then gently stretch your body."),
    ] {
        assert_eq!(
            full.trace.materials[&stage].preview(Language::English),
            expected
        );
    }
    for (limits, expected) in [
        (
            Limits {
                bytes: full.sentence.len() - 1,
                ..Limits::default()
            },
            GenerationError::ByteBudget,
        ),
        (
            Limits {
                atoms: full.trace.stages["28.passed"].observed - 1,
                ..Limits::default()
            },
            GenerationError::AtomBudget,
        ),
    ] {
        assert_eq!(
            generate_words(
                script.into_iter(),
                Language::English,
                TraceConfig::off(),
                limits
            )
            .unwrap_err(),
            expected
        );
    }
}

#[test]
fn trace_caps_reversible_transitions_and_lazy_order_do_not_change_selection() {
    for language in [Language::Chinese, Language::English] {
        for seed in [0, 1, 42, u64::MAX] {
            let collect = |config| {
                generate_words(SplitMix64::new(seed), language, config, Limits::default()).unwrap()
            };
            let full = collect(TraceConfig {
                enabled: true,
                previews: 64,
                events: 1024,
            });
            for config in [
                TraceConfig::default(),
                TraceConfig::off(),
                TraceConfig {
                    enabled: true,
                    previews: 0,
                    events: 0,
                },
            ] {
                let other = collect(config);
                assert_eq!((&full.sentence, full.draws), (&other.sentence, other.draws));
                if !config.enabled {
                    assert!(
                        other.trace.materials.is_empty(),
                        "plain generation must collect no material"
                    );
                }
                assert!(other.trace.events.len() <= 128);
                assert!(other
                    .trace
                    .stages
                    .values()
                    .all(|snapshot| snapshot.preview.len() <= 16));
            }
            let stages = &full.trace.stages;
            assert_eq!(stages["18.with_suffix"], stages["20.transition.unpaired"]);
            assert_eq!(stages["18.with_suffix"], stages["22.transition.unwrapped"]);
            assert_eq!(
                stages["23.transition.shuffled"].observed,
                stages["24.transition.restored"].observed
            );
            let events = &full.trace.events;
            let pairs: Vec<_> = events
                .iter()
                .enumerate()
                .filter(|(_, event)| event.stage == "15.paired_word")
                .map(|(i, _)| i)
                .collect();
            let fold = events
                .iter()
                .position(|event| event.stage == "34.accumulated")
                .unwrap();
            assert_eq!(pairs.len(), 2);
            assert!(pairs[0] < fold && fold < pairs[1]);
        }
    }
}
