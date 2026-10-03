use cli_common::{
    error,
    git::{self, Repo},
    local_time::LocalDay,
    Result,
};
use std::collections::HashSet;
use std::time::{Duration, Instant};

#[derive(Default)]
pub struct Totals {
    pub commits: u64,
    pub added: u64,
    pub deleted: u64,
    pub paths: HashSet<Vec<u8>>,
    pub binary_paths: HashSet<Vec<u8>>,
}
fn invalid() -> std::io::Error {
    error(
        "Git evidence is invalid or exceeds its budget",
        "Git 证据无效或超出预算",
    )
}
fn identity(repo: &Repo, field: &str) -> Result<Vec<u8>> {
    let mut value = git::run(&repo.root, ["config", "--null", "--get", field])?;
    if value.pop() != Some(0) || value.is_empty() || value.len() > 8192 {
        return Err(invalid());
    }
    // Exact raw configured name AND email, never a regex or mailmap match.
    if value.iter().all(|b| b.is_ascii_whitespace()) {
        return Err(invalid());
    }
    Ok(value)
}
pub fn collect(repo: &Repo, day: &LocalDay) -> Result<Totals> {
    let start = Instant::now();
    if git::run(&repo.root, ["rev-parse", "--is-shallow-repository"])? != b"false\n" {
        return Err(invalid());
    }
    let name = identity(repo, "user.name")?;
    let email = identity(repo, "user.email")?;
    // No --since traversal pruning: an older child can have a newer parent.
    // Pin the starting HEAD; never scan other refs or repositories.
    let log = git::run(
        &repo.root,
        [
            "-c",
            "log.showSignature=false",
            "-c",
            "i18n.logOutputEncoding=UTF-8",
            "log",
            "-z",
            "--format=%H%x00%ct%x00%an%x00%ae%x00%P",
            &repo.head,
            "--",
        ],
    )?;
    let fields: Vec<_> = log
        .strip_suffix(&[0])
        .ok_or_else(invalid)?
        .split(|b| *b == 0)
        .collect();
    if fields.len() % 5 != 0 || fields.len() / 5 > 20_000 {
        return Err(invalid());
    }
    let mut totals = Totals::default();
    let mut stat_bytes = 0usize;
    for entry in fields.chunks_exact(5) {
        if start.elapsed() > Duration::from_secs(10) {
            return Err(invalid());
        }
        let id = std::str::from_utf8(entry[0]).map_err(|_| invalid())?;
        if ![40, 64].contains(&id.len()) || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(invalid());
        }
        let timestamp = std::str::from_utf8(entry[1])
            .map_err(|_| invalid())?
            .parse::<i64>()
            .map_err(|_| invalid())?;
        if entry[2] != name || entry[3] != email || !day.contains(timestamp) {
            continue;
        }
        totals.commits += 1;
        if totals.commits > 1000 {
            return Err(invalid());
        }
        if entry[4]
            .split(|b| *b == b' ')
            .filter(|p| !p.is_empty())
            .count()
            > 1
        {
            continue;
        }
        // Rename detection deliberately off: old/new names count as distinct raw
        // paths. No external diff/textconv/filter execution; binaries have no LOC.
        let stat = git::run(
            &repo.root,
            [
                "-c",
                "diff.algorithm=myers",
                "diff-tree",
                "--root",
                "--no-commit-id",
                "--numstat",
                "-r",
                "-z",
                "--no-renames",
                "--no-ext-diff",
                "--no-textconv",
                id,
                "--",
            ],
        )?;
        stat_bytes = stat_bytes.saturating_add(stat.len());
        if stat_bytes > 32 * 1024 * 1024 {
            return Err(invalid());
        }
        add_stat(&mut totals, &stat)?;
    }
    if start.elapsed() > Duration::from_secs(10)
        || git::run(&repo.root, ["rev-parse", "--verify", "HEAD"])?
            != format!("{}\n", repo.head).as_bytes()
    {
        return Err(invalid());
    }
    Ok(totals)
}
fn add_stat(totals: &mut Totals, bytes: &[u8]) -> Result<()> {
    if !bytes.is_empty() && !bytes.ends_with(&[0]) {
        return Err(invalid());
    }
    for entry in bytes.split(|b| *b == 0).filter(|e| !e.is_empty()) {
        let fields: Vec<_> = entry.splitn(3, |b| *b == b'\t').collect();
        if fields.len() != 3 || fields[2].is_empty() {
            return Err(invalid());
        }
        totals.paths.insert(fields[2].to_vec());
        if fields[0] == b"-" && fields[1] == b"-" {
            totals.binary_paths.insert(fields[2].to_vec());
        } else {
            let count = |b| {
                std::str::from_utf8(b)
                    .map_err(|_| invalid())?
                    .parse::<u64>()
                    .map_err(|_| invalid())
            };
            totals.added = totals
                .added
                .checked_add(count(fields[0])?)
                .ok_or_else(invalid)?;
            totals.deleted = totals
                .deleted
                .checked_add(count(fields[1])?)
                .ok_or_else(invalid)?;
        }
        if totals.paths.len() > 20_000 {
            return Err(invalid());
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sums_text_deduplicates_raw_paths_and_does_not_invent_binary_lines() {
        let mut t = Totals::default();
        add_stat(&mut t, b"2\t1\tfile\0-\t-\tbinary\0").unwrap();
        add_stat(&mut t, b"3\t4\tfile\0-\t-\tbinary\0").unwrap();
        add_stat(&mut t, b"1\t0\tx\xff\t\n.rs\0").unwrap();
        assert_eq!(
            (t.added, t.deleted, t.paths.len(), t.binary_paths.len()),
            (6, 5, 3, 1)
        );
        assert!(add_stat(&mut t, b"-\t1\tx\0").is_err());
        assert!(add_stat(&mut t, b"1\t0\tunterminated").is_err());
    }
}
