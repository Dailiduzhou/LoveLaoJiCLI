const BIN: &str = env!("CARGO_BIN_EXE_stuck");

fn input_pty(long: bool) {
    let mut command = std::process::Command::new("python3");
    command
        .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/input_pty.py"))
        .arg(BIN);
    if long {
        command.arg("--long");
    }
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        cases::support::text(&output.stdout),
        cases::support::text(&output.stderr)
    );
}

#[test]
fn interactive_input_preserves_tool_boundaries() {
    input_pty(false);
}

#[test]
#[ignore = "known canonical-buffer overflow; shared terminal-input design approval required"]
fn long_interactive_input_contract() {
    input_pty(true);
}

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
