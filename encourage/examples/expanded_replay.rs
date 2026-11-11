//! Manual PTY driver for the current real chain; not an installed CLI.

use cli_common::{display, Language};
use encourage::{duration::PlaybackDuration, pipeline, tui};
use std::io::{self, Write};

fn main() {
    if let Err(error) = run() {
        let _ = writeln!(
            io::stderr().lock(),
            "encourage-replay: {}",
            display(error.to_string())
        );
        std::process::exit(1);
    }
}

fn run() -> cli_common::Result<()> {
    let language = Language::detect();
    let generation = pipeline::generate(0, language).map_err(io::Error::other)?;
    match tui::run(&generation, language, PlaybackDuration::default())? {
        tui::Outcome::Plain | tui::Outcome::Confirmed => {
            writeln!(io::stdout().lock(), "{}", generation.sentence)?;
        }
        tui::Outcome::Cancelled => {}
        tui::Outcome::Signal(signal) => {
            signal_hook::low_level::emulate_default_handler(signal)?;
            std::process::exit(128 + signal);
        }
    }
    Ok(())
}
