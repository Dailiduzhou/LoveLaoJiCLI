#![allow(dead_code)]
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

pub struct Fixture {
    pub root: PathBuf,
    pub repo: PathBuf,
    pub state: PathBuf,
    pub bin: &'static str,
    pub env: Vec<(&'static str, &'static str)>,
}
impl Fixture {
    pub fn new(bin: &'static str, git: bool) -> Self {
        let root = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("humanutils-test-{}", cli_common::state::id()));
        fs::create_dir(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        let repo = root.join("repo");
        let state = root.join("state");
        fs::create_dir(&repo).unwrap();
        fs::create_dir(&state).unwrap();
        let f = Self {
            root,
            repo,
            state,
            bin,
            env: Vec::new(),
        };
        if git {
            f.git(&["init", "-q"]);
            // Git runs maintenance in a detached process after commands that write
            // objects (maintenance.auto / gc.auto). It creates and removes
            // .git/objects/maintenance.lock at an unpredictable moment, which races
            // with the "nothing on disk changed" snapshots these fixtures assert.
            // Observed with git 2.56.0; a fixture repo needs no maintenance anyway.
            f.git(&["config", "gc.auto", "0"]);
            f.git(&["config", "maintenance.auto", "false"]);
            f.git(&["config", "user.name", "Test"]);
            f.git(&["config", "user.email", "test@localhost"]);
            fs::write(f.repo.join("code.rs"), b"fn main() {}\n").unwrap();
            f.git(&["add", "."]);
            f.git(&["commit", "-qm", "initial"]);
        }
        f
    }
    pub fn environment(&self, c: &mut Command) {
        c.current_dir(&self.repo)
            .env("PATH", "/usr/bin:/bin")
            .env("HOME", &self.root)
            .env("XDG_STATE_HOME", &self.state)
            .env("LC_ALL", "C")
            .envs(self.env.iter().copied())
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1");
        for k in [
            "GIT_DIR",
            "GIT_WORK_TREE",
            "GIT_INDEX_FILE",
            "GIT_COMMON_DIR",
            "GIT_CONFIG_COUNT",
            "GIT_CONFIG_PARAMETERS",
        ] {
            c.env_remove(k);
        }
    }
    pub fn cmd(&self) -> Command {
        let mut c = Command::new(self.bin);
        self.environment(&mut c);
        // spawn() inherits stdin by default, unlike output(). Isolate commands
        // from the test runner's pipe/file/terminal unless a test overrides it.
        c.stdin(Stdio::null());
        c
    }
    pub fn run(&self, args: &[&str]) -> Output {
        self.cmd().args(args).output().unwrap()
    }
    pub fn git(&self, args: &[&str]) -> Output {
        let mut c = Command::new("git");
        self.environment(&mut c);
        let o = c.args(args).output().unwrap();
        assert!(o.status.success(), "git {args:?}: {}", text(&o.stderr));
        o
    }
    pub fn store(&self) -> cli_common::state::Store {
        let root = self.state.join("lovelaojicli");
        let key = fs::read(root.join("identity.key"))
            .unwrap()
            .try_into()
            .unwrap();
        cli_common::state::Store { root, key }
    }
    pub fn find(&self, name: &str) -> Vec<PathBuf> {
        fn visit(p: &Path, name: &str, out: &mut Vec<PathBuf>) {
            if let Ok(entries) = fs::read_dir(p) {
                for e in entries.flatten() {
                    if e.file_type().unwrap().is_dir() {
                        visit(&e.path(), name, out)
                    } else if e.file_name() == name {
                        out.push(e.path())
                    }
                }
            }
        }
        let mut out = vec![];
        visit(&self.state, name, &mut out);
        out
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
pub fn text(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}
pub fn code(o: &Output, n: i32) {
    assert_eq!(
        o.status.code(),
        Some(n),
        "stdout={} stderr={}",
        text(&o.stdout),
        text(&o.stderr)
    );
}
pub fn basics(bin: &'static str, tool: &str) {
    let f = Fixture::new(bin, false);
    for flag in ["--version", "--verison", "-V"] {
        let o = f.run(&[flag]);
        code(&o, 0);
        assert_eq!(
            text(&o.stdout),
            format!("{tool} {}\n", env!("CARGO_PKG_VERSION"))
        );
    }
    for locale in ["C", "zh_CN.UTF-8"] {
        let o = f
            .cmd()
            .arg("--help")
            .env("LC_ALL", locale)
            .output()
            .unwrap();
        code(&o, 0);
        let t = text(&o.stdout);
        assert!(t.contains(if locale == "C" { "Usage" } else { "用法" }));
    }
    code(&f.run(&["--unknown"]), 2);
}

/// Custom validation diagnostics follow the same locale priority as help.
pub fn localized_usage_error(f: &Fixture, args: &[&str], english: &str, chinese: &str) {
    for (all, messages, lang, expected) in [
        ("en_US", "zh_CN", "zh_CN", english),
        ("zh_CN", "en_US", "en_US", chinese),
        ("", "zh_CN", "en_US", chinese),
        ("", "", "zh-CN", chinese),
    ] {
        let o = f
            .cmd()
            .env("LC_ALL", all)
            .env("LC_MESSAGES", messages)
            .env("LANG", lang)
            .args(args)
            .output()
            .unwrap();
        code(&o, 2);
        assert!(o.stdout.is_empty());
        assert!(text(&o.stderr).contains(expected), "{}", text(&o.stderr));
    }
    assert_eq!(fs::read_dir(&f.state).unwrap().count(), 0);
}
