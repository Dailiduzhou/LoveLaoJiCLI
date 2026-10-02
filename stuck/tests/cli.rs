const BIN: &str = env!("CARGO_BIN_EXE_stuck");
const TOOL: &str = "stuck";
#[path = "../../tests/repeat_cases.rs"]
mod cases;
#[test]
fn validates_hypothesis() {
    let f = cases::support::Fixture::new(BIN, false);
    for s in [" ", "a\nb", &"x".repeat(513)] {
        cases::support::code(&f.run(&["--hypothesis", s, "true"]), 2);
    }
}
