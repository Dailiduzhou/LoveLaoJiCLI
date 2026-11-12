use encourage::rng::{accepted_index, encode_argv, seed_from_argv, Attempts, Budgeted, SplitMix64};
use encourage::GenerationError;

#[test]
fn splitmix64_known_vectors_and_wraparound() {
    assert_eq!(
        SplitMix64::new(0).take(5).collect::<Vec<_>>(),
        vec![
            0xe220_a839_7b1d_cdaf,
            0x6e78_9e6a_a1b9_65f4,
            0x06c4_5d18_8009_454f,
            0xf88b_b8a8_724c_81ec,
            0x1b39_896a_51a8_749b,
        ]
    );
    // Deliberately independent, widened-arithmetic formulation.
    let mask = u64::MAX as u128;
    let mut state = u64::MAX as u128;
    let mut actual = SplitMix64::new(u64::MAX);
    for _ in 0..1000 {
        state = (state + 0x9E37_79B9_7F4A_7C15_u128) & mask;
        let mut z = state;
        z = ((z ^ (z >> 30)) * 0xBF58_476D_1CE4_E5B9_u128) & mask;
        z = ((z ^ (z >> 27)) * 0x94D0_49BB_1331_11EB_u128) & mask;
        assert_eq!(actual.next(), Some((z ^ (z >> 31)) as u64));
    }
}

#[test]
fn argv_encoding_is_length_prefixed_utf8_without_normalization() {
    let mut expected = b"encourage/argv/v1\0".to_vec();
    expected.extend_from_slice(&2_u64.to_le_bytes());
    expected.extend_from_slice(&1_u64.to_le_bytes());
    expected.push(b'a');
    expected.extend_from_slice(&3_u64.to_le_bytes());
    expected.extend_from_slice("饭".as_bytes());
    assert_eq!(encode_argv(&["a", "饭"]).unwrap(), expected);
    for (a, b) in [
        (vec!["a", "b"], vec!["a b"]),
        (vec!["a"], vec!["A"]),
        (vec!["a"], vec![" a"]),
        (vec!["é"], vec!["e\u{301}"]),
        (vec![""], vec!["", ""]),
        (vec!["a\0b"], vec!["a", "b"]),
    ] {
        assert_ne!(encode_argv(&a).unwrap(), encode_argv(&b).unwrap());
        // Specific regression vectors, not a claim of collision freedom.
        assert_ne!(seed_from_argv(&a).unwrap(), seed_from_argv(&b).unwrap());
    }
    assert_eq!(encode_argv(&[]), Err(GenerationError::NoExplicitArguments));
    assert!(seed_from_argv(&[""]).is_ok());
}

#[test]
fn argv_seed_golden_vectors() {
    for (args, expected) in [
        (vec![""], 0xf271_73f5_2ec4_7b20),
        (vec!["hello"], 0x06fb_e595_ebd1_a095),
        (vec!["今天有点累"], 0x6046_17da_3b67_72e8),
        (vec!["a", "b"], 0x77b0_793e_4946_8a01),
        (vec!["a b"], 0x6d0a_0321_17df_ae19),
        (vec!["42"], 0xb6af_cb86_0bc2_0ce9),
    ] {
        assert_eq!(seed_from_argv(&args), Ok(expected));
    }
}

#[test]
fn rejection_and_singleton_draw_contract() {
    assert_eq!(accepted_index(0, 3).unwrap(), None);
    assert_eq!(accepted_index(1, 3).unwrap(), Some(1));
    assert_eq!(accepted_index(u64::MAX, 3).unwrap(), Some(0));
    assert_eq!(accepted_index(0, 2).unwrap(), Some(0));
    assert_eq!(accepted_index(0, 1).unwrap(), Some(0));
    assert_eq!(accepted_index(0, 0), Err(GenerationError::EmptyCandidates));
    let mut rng = Budgeted::new([0, 1, 99].into_iter(), 3);
    assert_eq!(rng.choose(3), Ok(1));
    assert_eq!(rng.used, 2);
    assert_eq!(rng.choose(1), Ok(0));
    assert_eq!(rng.used, 3);
    assert_eq!(rng.word(), Err(GenerationError::DrawBudget));
}

#[test]
fn zero_bound_does_not_consume_a_word() {
    let mut rng = Budgeted::new([8].into_iter(), 10);
    assert_eq!(rng.choose(0), Err(GenerationError::EmptyCandidates));
    assert_eq!(rng.used, 0);
    assert_eq!(
        Attempts::new(&mut rng, &[0]).next(),
        Some(Err(GenerationError::EmptyCandidates))
    );
    assert_eq!(rng.used, 0);
}

#[test]
fn eight_bit_exhaustive_model_exposes_modulo_bias_and_removes_it() {
    for n in 1..=255_u16 {
        let threshold = 256 % n;
        let mut counts = vec![0; n as usize];
        for raw in threshold..256 {
            counts[(raw % n) as usize] += 1;
        }
        assert!(counts.iter().all(|&count| count == counts[0]));
    }
    let mut modulo = [0; 3];
    let mut rejected = [0; 3];
    for raw in 0..256 {
        modulo[raw % 3] += 1;
        if raw >= 1 {
            rejected[raw % 3] += 1;
        }
    }
    assert_eq!(modulo, [86, 85, 85]);
    assert_eq!(rejected, [85, 85, 85]);
}

#[test]
fn rejection_does_not_advance_slots_and_end_does_not_overdraw() {
    let mut rng = Budgeted::new([0, 1, 0, 99].into_iter(), 10);
    let attempts = Attempts::new(&mut rng, &[3, 2])
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(
        attempts
            .iter()
            .map(|a| (a.slot, a.index))
            .collect::<Vec<_>>(),
        vec![(0, None), (0, Some(1)), (1, Some(0))]
    );
    assert_eq!(rng.used, 3);
    assert_eq!(rng.word(), Ok(99));
}

#[test]
fn bounded_failure_is_an_error_not_a_fallback() {
    let mut budget = Budgeted::new(std::iter::repeat(0), 7);
    assert_eq!(budget.choose(3), Err(GenerationError::DrawBudget));
    assert_eq!(budget.used, 7);
    let mut ended = Budgeted::new([0].into_iter(), 7);
    assert_eq!(ended.choose(3), Err(GenerationError::RandomSourceEnded));
}
