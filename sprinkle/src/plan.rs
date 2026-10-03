//! Build file fingerprints and an exact insertion patch before mutating files.
use crate::manifest::{FileRecord, Manifest};
use cli_common::{
    git::{self, Repo},
    state, Language, Result,
};
use rand::{Rng, SeedableRng};
use std::{
    fs,
    os::unix::{ffi::OsStrExt, fs::MetadataExt},
    path::Path,
};

pub(super) fn plan(copy: &Repo, target: &Path, m: &mut Manifest, l: Language) -> Result<usize> {
    let seed = std::env::var("SPRINKLE_SEED")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or_else(rand::random);
    let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
    let mut files = copy.tracked()?;
    files.sort_by(|a, b| a.0.cmp(&b.0));
    let mut count = 0;
    for (p, mode) in files {
        m.scanned += 1;
        let path = git::safe_path(target, &p)?;
        let metadata = fs::symlink_metadata(&path)?;
        let before = if metadata.file_type().is_symlink() {
            state::digest(fs::read_link(&path)?.as_os_str().as_bytes())
        } else {
            git::fingerprint(&path)?
        };
        let mut record = FileRecord {
            path: p.clone(),
            before: before.clone(),
            after: before,
            mode: metadata.mode() & 0o177777,
            offset: 0,
            addition: String::new(),
            message: 0,
        };
        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
        let reason = if mode == "120000" {
            Some("symlink")
        } else if p.split(|c| *c == b'/').any(|c| {
            [
                b".git".as_slice(),
                b"target",
                b"vendor",
                b"node_modules",
                b"dist",
                b"build",
                b"third_party",
                b"generated",
            ]
            .contains(&c)
        }) {
            Some("excluded-directory")
        } else if ![
            "md", "go", "rs", "c", "h", "cc", "cpp", "cxx", "hh", "hpp", "hxx",
        ]
        .contains(&ext)
        {
            Some("extension")
        } else if metadata.len() > 1024 * 1024 {
            Some("size")
        } else {
            None
        };
        if let Some(reason) = reason {
            *m.skipped.entry(reason.into()).or_default() += 1;
            m.files.push(record);
            continue;
        }
        let bytes = fs::read(&path)?;
        let text = std::str::from_utf8(&bytes).ok();
        if !text.is_some_and(|t| crate::scan::safe(t, ext)) {
            *m.skipped.entry("syntax-or-text".into()).or_default() += 1;
            m.files.push(record);
            continue;
        }
        if rng.gen_bool(0.25) && count < 100 {
            let messages=l.text("you're doing fine|one thing at a time|this can wait|good enough is good|take your time|future-you says hi|one weird bug at a time|remember water|breathe.|tomorrow is also available", "慢一点也可以|先解决一个|今天可以到这里|能用已经很好|不着急|明天的你说你好|一次处理一个奇怪的 bug|喝口水|深呼吸|明天也可以处理");
            let messages: Vec<_> = messages.split('|').collect();
            let choice = rng.gen_range(0..messages.len());
            let newline = if text.unwrap().contains("\r\n") {
                "\r\n"
            } else {
                "\n"
            };
            let comment = if ext == "md" {
                format!("<!-- {} -->", messages[choice])
            } else if ["rs", "go"].contains(&ext) {
                format!("// {}", messages[choice])
            } else {
                format!("/* {} */", messages[choice])
            };
            record.addition = format!("{newline}{comment}{newline}");
            record.offset = bytes.len();
            record.message = choice;
            let mut after = bytes;
            after.extend_from_slice(record.addition.as_bytes());
            record.after = state::digest(&after);
            count += 1;
        }
        m.files.push(record);
    }
    // Save the exact zero-context patch before touching files. Git-style quoting is byte-safe.
    for f in m.files.iter().filter(|f| !f.addition.is_empty()) {
        let before = fs::read(git::safe_path(target, &f.path)?)?;
        let lines = before.iter().filter(|b| **b == b'\n').count();
        let a = quote_path(b"a/", &f.path);
        let b = quote_path(b"b/", &f.path);
        m.patch.push_str(&format!(
            "diff --git {a} {b}\n--- {a}\n+++ {b}\n@@ -{lines},0 +{},2 @@\n",
            lines + 1
        ));
        for line in f.addition.split_inclusive('\n') {
            m.patch.push('+');
            m.patch.push_str(line);
        }
    }
    m.patch_hash = state::digest(m.patch.as_bytes());
    Ok(count)
}

fn quote_path(prefix: &[u8], p: &[u8]) -> String {
    let mut s = String::from("\"");
    for b in prefix.iter().chain(p) {
        match b {
            b'"' => s.push_str("\\\""),
            b'\\' => s.push_str("\\\\"),
            32..=126 => s.push(*b as char),
            _ => s.push_str(&format!("\\{b:03o}")),
        }
    }
    s.push('"');
    s
}

#[cfg(test)]
mod tests {
    use super::quote_path;

    #[test]
    fn patch_paths_preserve_spaces_quotes_controls_and_non_utf8_bytes() {
        assert_eq!(quote_path(b"a/", b"plain file.rs"), "\"a/plain file.rs\"");
        assert_eq!(
            quote_path(b"b/", b"quote\"slash\\tab\tline\n\xff.rs"),
            "\"b/quote\\\"slash\\\\tab\\011line\\012\\377.rs\""
        );
    }
}
