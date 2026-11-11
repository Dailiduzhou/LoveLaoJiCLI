use super::*;

#[test]
fn only_absent_arguments_read_entropy_and_failures_never_fall_back() {
    for args in [vec![""], vec!["hello"], vec!["a", "b"]] {
        let actual = seed_with_entropy(&args, |_| panic!("explicit text requested entropy"));
        assert_eq!(actual.unwrap(), seed_from_argv(&args).unwrap());
    }
    let actual = seed_with_entropy(&[], |bytes| {
        *bytes = [1, 2, 3, 4, 5, 6, 7, 8];
        Ok(())
    });
    assert_eq!(actual.unwrap(), 0x0807_0605_0403_0201);
    let error = seed_with_entropy(&[], |_| Err(io::Error::other("entropy unavailable")));
    assert_eq!(error.unwrap_err().to_string(), "entropy unavailable");
}
