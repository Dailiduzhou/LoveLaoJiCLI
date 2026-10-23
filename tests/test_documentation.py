"""Language-separated README contracts and isolated release-version synchronization."""
from pathlib import Path
import re
import shlex
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]


def shell_examples(content):
    commands = []
    for block in re.findall(r"```sh\n(.*?)```", content, re.S):
        for line in block.replace("\\\n", " ").splitlines():
            tokens = shlex.split(line, comments=True)
            # Localized example names do not change the documented interface.
            if tokens[:2] in (["poke", "add"], ["poke", "remove"]):
                tokens[2] = "<name>"
            if tokens:
                commands.append(tokens)
    return commands


class ReadmeTests(unittest.TestCase):
    def test_language_navigation_and_local_links(self):
        english_files = [ROOT / "README.md", *sorted(ROOT.glob("*/README.md"))]
        for english in english_files:
            chinese = english.with_name("README-zh.md")
            with self.subTest(readme=str(english.relative_to(ROOT))):
                self.assertTrue(chinese.is_file())
                self.assertIn("](README-zh.md)", english.read_text())
                self.assertIn("](README.md)", chinese.read_text())
            for path in (english, chinese):
                for target in re.findall(r"\[[^\]]*\]\(([^)]+)\)", path.read_text()):
                    if "://" in target or target.startswith("#"):
                        continue
                    with self.subTest(source=str(path), target=target):
                        self.assertTrue((path.parent / target.split("#", 1)[0]).exists())

    def test_root_sections_and_shell_examples_match(self):
        english = (ROOT / "README.md").read_text()
        chinese = (ROOT / "README-zh.md").read_text()
        self.assertEqual(re.findall(r"^(#+) ", english, re.M),
                         re.findall(r"^(#+) ", chinese, re.M))
        self.assertEqual(shell_examples(english), shell_examples(chinese))
        self.assertNotRegex(english, re.compile(r"^#{2,} .* / ", re.M), "headings must not be bilingual")
        # Chinese navigation and literal CLI output are intentional; prose must
        # remain English. Link targets may refer to Chinese roadmap anchors.
        prose = re.sub(r"```.*?```", "", english, flags=re.S)
        prose = re.sub(r"\]\([^)]*\)", "]", prose).replace("[简体中文]", "")
        self.assertNotRegex(prose, r"[\u3400-\u9fff]")


class VersionSyncTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="readme-version-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        self.old, self.new = "1.2.3", "1.2.4"
        (self.root / "scripts").mkdir()
        shutil.copy2(ROOT / "scripts/bump-version.sh", self.root / "scripts/bump-version.sh")
        (self.root / "demo/src").mkdir(parents=True)
        (self.root / "Cargo.toml").write_text(
            '[workspace]\nmembers = ["demo"]\nresolver = "2"\n'
            f'[workspace.package]\nversion = "{self.old}"\n')
        (self.root / "demo/Cargo.toml").write_text(
            '[package]\nname = "demo"\nversion.workspace = true\nedition = "2021"\n')
        (self.root / "demo/src/main.rs").write_text("fn main() {}\n")
        (self.root / "install.sh").write_text(f'RELEASE_VERSION=${{LOVELAOJI_VERSION:-v{self.old}}}\n')
        for name in ("README.md", "README-zh.md"):
            (self.root / name).write_text(f"Release: `v{self.old}`\n")
        (self.root / "ROADMAP.md").write_text(
            f"历史基线：`0.1.0`\n当前开发版本为 `{self.old}`\nworkspace version: `{self.old}`\n")
        self.run_command(["git", "init", "-q"])
        self.run_command(["cargo", "generate-lockfile", "--offline"])

    def run_command(self, args):
        return subprocess.run(args, cwd=self.root, capture_output=True, text=True,
                              timeout=30, check=True)

    def test_bump_updates_both_readmes_lockfile_and_current_version_only(self):
        # Use the platform's real Bash, including macOS /bin/bash 3.2.
        self.run_command(["/bin/bash", "scripts/bump-version.sh", f"v{self.new}"])
        for name in ("README.md", "README-zh.md", "install.sh"):
            content = (self.root / name).read_text()
            self.assertIn(f"v{self.new}", content)
            self.assertNotIn(f"v{self.old}", content)
        for name in ("Cargo.toml", "Cargo.lock"):
            self.assertIn(f'version = "{self.new}"', (self.root / name).read_text())
        roadmap = (self.root / "ROADMAP.md").read_text()
        self.assertIn("历史基线：`0.1.0`", roadmap)
        self.assertNotIn(self.old, roadmap)
        self.assertFalse(list(self.root.rglob("*.bak")))
        self.run_command(["/bin/bash", "scripts/bump-version.sh", f"v{self.new}"])


if __name__ == "__main__":
    unittest.main()
