use encourage_experiments::rng::{Attempts, Budgeted};
use std::cell::{Cell, RefCell};

#[test]
fn take_while_consumes_the_stopping_item_but_not_the_tail() {
    let seen = RefCell::new(Vec::new());
    let mut source = (0..8).inspect(|x| seen.borrow_mut().push(*x));
    let output: Vec<_> = source.by_ref().take_while(|&x| x < 3).collect();
    assert_eq!(output, [0, 1, 2]);
    assert_eq!(*seen.borrow(), [0, 1, 2, 3]);
    assert_eq!(source.next(), Some(4));
}

#[test]
fn zip_can_pull_an_unpaired_left_item() {
    let pulls = Cell::new(0);
    let mut left = (0..10).inspect(|_| pulls.set(pulls.get() + 1));
    {
        let mut zipped = left.by_ref().zip(["a", "b"]);
        assert_eq!(zipped.next(), Some((0, "a")));
        assert_eq!(zipped.next(), Some((1, "b")));
        assert_eq!(zipped.next(), None);
    }
    assert_eq!(pulls.get(), 3);
    assert_eq!(left.next(), Some(3));
}

#[test]
fn filtering_after_zip_corrupts_slot_alignment() {
    let mut wrong_rng = Budgeted::new([0, 1, 0].into_iter(), 10);
    let wrong: Vec<_> = Attempts::new(&mut wrong_rng, &[3, 2])
        .zip(["first", "second"])
        .filter_map(|(a, name)| {
            let a = a.unwrap();
            a.index.map(|index| (a.slot, name, index))
        })
        .collect();
    assert_eq!(wrong, [(0, "second", 1)]);

    let mut correct_rng = Budgeted::new([0, 1, 0].into_iter(), 10);
    let correct: Vec<_> = Attempts::new(&mut correct_rng, &[3, 2])
        .filter_map(|a| {
            let a = a.unwrap();
            a.index.map(|index| (a.slot, index))
        })
        .zip(["first", "second"])
        .collect();
    assert_eq!(correct, [((0, 1), "first"), ((1, 0), "second")]);
}

#[test]
#[expect(
    clippy::manual_try_fold,
    reason = "explicit fold/try_fold behavior experiment"
)]
fn fold_of_result_keeps_pulling_where_try_fold_stops() {
    let fold_pulls = Cell::new(0);
    let folded: Result<i32, &str> = (0..5)
        .inspect(|_| fold_pulls.set(fold_pulls.get() + 1))
        .map(|x| if x == 1 { Err("failure") } else { Ok(x) })
        .fold(Ok(0), |sum, item| Ok(sum? + item?));
    assert_eq!(folded, Err("failure"));
    assert_eq!(fold_pulls.get(), 5);

    let try_pulls = Cell::new(0);
    let folded: Result<i32, &str> = (0..5)
        .inspect(|_| try_pulls.set(try_pulls.get() + 1))
        .map(|x| if x == 1 { Err("failure") } else { Ok(x) })
        .try_fold(0, |sum, item| Ok(sum + item?));
    assert_eq!(folded, Err("failure"));
    assert_eq!(try_pulls.get(), 2);
}

#[test]
fn per_item_pull_order_is_not_stage_by_stage_order() {
    let calls = RefCell::new(Vec::new());
    let output = (0..3)
        .map(|x| {
            calls.borrow_mut().push(("map", x));
            x * 2
        })
        .filter(|x| {
            calls.borrow_mut().push(("filter", *x));
            *x != 2
        })
        .fold(Vec::new(), |mut output, x| {
            calls.borrow_mut().push(("fold", x));
            output.push(x);
            output
        });
    assert_eq!(output, [0, 4]);
    assert_eq!(
        *calls.borrow(),
        [
            ("map", 0),
            ("filter", 0),
            ("fold", 0),
            ("map", 1),
            ("filter", 2),
            ("map", 2),
            ("filter", 4),
            ("fold", 4),
        ]
    );
}
