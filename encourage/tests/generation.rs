use encourage::{
    corpus,
    pipeline::{self, CONTENT_OPERATION_COUNT, GENERATION_VERSION, OPERATIONS, TRANSITION_STAGES},
    revision, rng, Language,
};
use std::collections::BTreeSet;

#[test]
fn generation_v2_retains_v1_text_across_10000_bilingual_seeds() {
    let mut covered = [BTreeSet::new(), BTreeSet::new()];
    for seed in 0..10_000 {
        let mut frame = None;
        for (i, language) in [Language::Chinese, Language::English]
            .into_iter()
            .enumerate()
        {
            let expected = revision::reference(seed, language).unwrap();
            let sentence = pipeline::generate_plain(seed, language).unwrap();
            assert_eq!(sentence, expected.1);
            if let Some(index) = frame {
                assert_eq!(expected.0, index);
            }
            frame = Some(expected.0);
            covered[i].insert(sentence);
        }
    }
    assert_eq!(covered.map(|set| set.len()), [34, 34]);
}

#[test]
fn protocol_golden_sentences_and_trace_repeat_exactly() {
    assert_eq!(GENERATION_VERSION, "encourage-gen-v2");
    for (args, language, sentence) in [
        (
            vec!["42"],
            Language::English,
            "Have some cool water, then slowly stretch your body.",
        ),
        (
            vec!["今天有点累"],
            Language::Chinese,
            "困了就休息一会儿，不必一直赶路。",
        ),
    ] {
        // The seed vectors are separately frozen in rng_contract.rs. These
        // cases also check public traced/plain paths against the same oracle.
        let seed = rng::seed_from_argv(&args).unwrap();
        let first = pipeline::generate(seed, language).unwrap();
        let second = pipeline::generate(seed, language).unwrap();
        assert_eq!(first, second);
        assert_eq!(
            first.sentence,
            pipeline::generate_plain(seed, language).unwrap()
        );
        assert_eq!(first.sentence, sentence);
    }
    assert_eq!(
        pipeline::generate_plain(0, Language::English).unwrap(),
        "You deserve patience, even when things seem difficult."
    );
    assert_eq!(
        pipeline::generate_plain(0, Language::Chinese).unwrap(),
        "这次没做成，也值得被温柔对待。"
    );
}

#[test]
fn every_reviewed_combination_and_candidate_order_is_frozen() {
    let mut snapshot = String::new();
    for language in [Language::Chinese, Language::English] {
        for (index, frame) in corpus::frames(language).iter().enumerate() {
            for a in 0..frame.slots[0].variants.len() {
                for b in 0..frame.slots[1].variants.len() {
                    snapshot.push_str(&revision::render_choices(index, language, &[a, b]).unwrap());
                    snapshot.push('\n');
                }
            }
        }
    }
    assert_eq!(snapshot, include_str!("fixtures/gen-v1.txt"));
}

#[test]
fn typed_materials_are_real_stage_outputs_not_cleaned_debug_strings() {
    for language in [Language::Chinese, Language::English] {
        for seed in 0..128 {
            let generation = pipeline::generate(seed, language).unwrap();
            let stages = &generation.trace.stages;
            for stage in &stages[..10] {
                assert!(stage.material.raw().is_empty());
                assert!(stage.material.preview(language).is_empty());
            }
            for stage in &stages[10..15] {
                assert!(!stage.material.raw().is_empty());
                assert!(
                    stage.material.preview(language).is_empty(),
                    "alternatives are not a sentence"
                );
            }
            for stage in stages {
                let raw = stage.material.raw();
                for wrapper in [
                    "Word(",
                    "Atom",
                    "Ok(",
                    "Origin",
                    "slot=",
                    "transition.",
                    "End",
                ] {
                    assert!(!raw.contains(wrapper), "{}: {raw}", stage.id);
                }
                assert_eq!(stage.material.omitted(), 0);
            }
            assert_ne!(stages[16].material.preview(language), generation.sentence);
            let base = &stages[17].material;
            for number in [19, 20, 21, 22, 24, 25] {
                assert_eq!(&stages[number - 1].material, base);
            }
            assert_eq!(stages[26].material, stages[27].material);
            assert_eq!(stages[27].material, stages[28].material);
            for number in [33, 34] {
                assert_eq!(
                    stages[number - 1].material.preview(language),
                    generation.sentence
                );
            }
            assert_eq!(stages[33].material.pieces().len(), 1);
            assert_eq!(stages[33].material.raw(), generation.sentence);
        }
    }
}

#[test]
fn one_real_chain_keeps_content_and_transitions_distinct() {
    let source = include_str!("../src/pipeline.rs");
    let display = source
        .split("// encourage:display:start")
        .nth(1)
        .unwrap()
        .split("// encourage:display:end")
        .next()
        .unwrap();
    assert_eq!(OPERATIONS.len(), 34);
    assert_eq!(CONTENT_OPERATION_COUNT, 28);
    assert_eq!(TRANSITION_STAGES, [19, 20, 21, 22, 23, 24]);
    assert_eq!(display.matches("// op").count(), 34);
    assert_eq!(display.matches(".fold(").count(), 1);
    for (index, (method, description)) in OPERATIONS.iter().enumerate() {
        let marker = format!("// op{:02}: {method}", index + 1);
        assert_eq!(display.matches(&marker).count(), 1);
        let (comment, code) = display
            .split(&marker)
            .nth(1)
            .unwrap()
            .split_once('\n')
            .unwrap();
        let transition = TRANSITION_STAGES.contains(&(index + 1));
        assert_eq!(comment.contains("TRANSITION"), transition);
        assert_eq!(description.starts_with("TRANSITION"), transition);
        assert!(code.trim_start().starts_with(&format!(".{method}(")));
    }
}
