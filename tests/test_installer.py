"""Isolated installer tests; never edit the real HOME or compile real binaries."""

import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

SCRIPT = Path(__file__).resolve().parents[1] / "install.sh"
BEGIN = "# >>> LoveLaoJiCLI PATH >>>"
END = "# <<< LoveLaoJiCLI PATH <<<"


class InstallerTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="lovelaojicli-test-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.home = self.root / "home with 'quote and $dollar"
        self.home.mkdir()
        self.project = self.root / "project with spaces"
        self.project.mkdir()
        shutil.copy2(SCRIPT, self.project / "install.sh")
        self.fake_bin = self.root / "fake-bin"
        self.fake_bin.mkdir()
        self.executable("rustc", "#!/bin/sh\nprintf 'host: test-host\\n'\n")
        self.executable("cargo", """#!/usr/bin/env bash
set -eu
[[ ${FAIL_BUILD:-0} == 0 ]] || exit 42
while [[ $# -gt 0 ]]; do
    case "$1" in
        --target-dir) target=$2; shift ;;
        --target) host=$2; shift ;;
    esac
    shift
done
mkdir -p "$target/$host/release"
for tool in love happiness joy; do
    printf '#!/bin/sh\\nprintf "%s 0.1.0\\\\n"\\n' "$tool" > "$target/$host/release/$tool"
    chmod +x "$target/$host/release/$tool"
done
""")
        self.env = os.environ.copy()
        for key in ["LC_ALL", "LC_MESSAGES", "LANGUAGE", "XDG_DATA_HOME", "XDG_CONFIG_HOME", "ZDOTDIR", "BASH_ENV", "ENV"]:
            self.env.pop(key, None)
        self.env.update(HOME=str(self.home), SHELL="/bin/bash", LANG="C",
                        PATH=f"{self.fake_bin}:/usr/bin:/bin")
        self.install_dir = self.home / ".local/share/lovelaojicli"
        self.bin_dir = self.install_dir / "bin"

    def executable(self, name, content):
        path = self.fake_bin / name
        path.write_text(content)
        path.chmod(0o755)

    def run_script(self, *args, answer="y\n", ok=True):
        result = subprocess.run(["bash", str(self.project / "install.sh"), *args],
                                input=answer, text=True, capture_output=True,
                                cwd=self.root, env=self.env, timeout=20)
        if ok:
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        else:
            self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
        return result

    def test_install_idempotent_path_and_safe_uninstall(self):
        bashrc = self.home / ".bashrc"
        original = "# user configuration\nexport KEEP_ME=yes\n"
        bashrc.write_text(original)
        bashrc.chmod(0o640)
        self.run_script(answer="1\ny\n")
        first = bashrc.read_text()
        self.run_script("install")
        self.assertEqual(bashrc.read_text(), first)
        self.assertEqual(first.count(BEGIN), 1)
        self.assertEqual(bashrc.stat().st_mode & 0o777, 0o640)
        self.assertIn(BEGIN, (self.home / ".profile").read_text())
        result = subprocess.run(
            ["bash", "--noprofile", "--norc", "-c",
             '. "$HOME/.bashrc"; . "$HOME/.bashrc"; command -v love; love; printf "%s\\n" "$PATH"'],
            env=self.env, text=True, capture_output=True, check=True)
        self.assertEqual(result.stdout.splitlines()[0], str(self.bin_dir / "love"))
        self.assertIn("love 0.1.0", result.stdout)
        self.assertEqual(result.stdout.splitlines()[-1].split(":").count(str(self.bin_dir)), 1)
        unrelated = self.bin_dir / "keep-me"
        unrelated.write_text("untouched")
        self.run_script("uninstall")
        self.assertEqual(bashrc.read_text(), original)
        self.assertEqual(unrelated.read_text(), "untouched")
        for tool in ["love", "happiness", "joy"]:
            self.assertFalse((self.bin_dir / tool).exists())
        self.run_script("uninstall")

    def test_cancel_and_eof_do_not_install(self):
        for args, answer in [((), "0\n"), ((), ""), (("install",), "n\n"), (("install",), "")]:
            self.run_script(*args, answer=answer)
            self.assertFalse(self.install_dir.exists())
            self.assertFalse((self.home / ".bashrc").exists())

    def test_build_failure_does_not_modify_home(self):
        self.env["FAIL_BUILD"] = "1"
        self.run_script("install", ok=False)
        self.assertEqual(list(self.home.iterdir()), [])

    def test_failed_upgrade_preserves_installation(self):
        self.run_script("install")
        before = (self.bin_dir / "love").read_bytes()
        self.env["FAIL_BUILD"] = "1"
        self.run_script("install", ok=False)
        self.assertEqual((self.bin_dir / "love").read_bytes(), before)

    def test_existing_unmanaged_install_is_not_overwritten(self):
        self.install_dir.mkdir(parents=True)
        sentinel = self.install_dir / "mine"
        sentinel.write_text("keep")
        self.run_script("install", ok=False)
        self.assertEqual(sentinel.read_text(), "keep")

    def test_malformed_block_is_not_modified(self):
        bashrc = self.home / ".bashrc"
        original = f"# keep\n{BEGIN}\nunfinished\n"
        bashrc.write_text(original)
        self.run_script("install", ok=False)
        self.assertEqual(bashrc.read_text(), original)
        self.assertFalse(self.install_dir.exists())

    def test_binary_directory_is_not_overwritten(self):
        self.run_script("install")
        binary = self.bin_dir / "love"
        binary.unlink()
        binary.mkdir()
        self.run_script("install", ok=False)
        self.assertEqual(list(binary.iterdir()), [])

    def test_bash_login_profile_selection(self):
        for name in [".bash_profile", ".bash_login"]:
            with self.subTest(name=name):
                profile = self.home / name
                profile.write_text("# login\n")
                self.run_script("install")
                self.assertIn(BEGIN, profile.read_text())
                self.assertFalse((self.home / ".profile").exists())
                self.run_script("uninstall")
                self.assertEqual(profile.read_text(), "# login\n")
                profile.unlink()

    def test_symlinked_dotfile_is_preserved(self):
        target = self.home / "dotfiles/bashrc"
        target.parent.mkdir()
        target.write_text("# dotfiles\n")
        link = self.home / ".bashrc"
        link.symlink_to(target)
        self.run_script("install")
        self.assertTrue(link.is_symlink())
        self.assertIn(BEGIN, target.read_text())
        self.run_script("uninstall")
        self.assertEqual(target.read_text(), "# dotfiles\n")

    def test_zsh_custom_location_and_uninstall_after_shell_change(self):
        self.env.update(SHELL="/bin/zsh", ZDOTDIR=str(self.home / "zsh configs"))
        self.run_script("install")
        files = [Path(self.env["ZDOTDIR"]) / name for name in [".zshrc", ".zprofile"]]
        for file in files:
            self.assertIn(BEGIN, file.read_text())
        self.env["SHELL"] = "/bin/bash"
        self.run_script("uninstall")
        for file in files:
            self.assertNotIn(BEGIN, file.read_text())

    def test_fish_configuration(self):
        self.env.update(SHELL="/usr/bin/fish", XDG_CONFIG_HOME=str(self.home / "custom config"))
        self.run_script("install")
        config = Path(self.env["XDG_CONFIG_HOME"]) / "fish/conf.d/lovelaojicli.fish"
        self.assertIn("set -gx PATH", config.read_text())
        if shutil.which("fish"):
            result = subprocess.run(
                [shutil.which("fish"), "--no-config", "-c",
                 'source "$XDG_CONFIG_HOME/fish/conf.d/lovelaojicli.fish"; source "$XDG_CONFIG_HOME/fish/conf.d/lovelaojicli.fish"; command -s love; love'],
                env=self.env, text=True, capture_output=True, check=True)
            self.assertEqual(result.stdout.splitlines()[0], str(self.bin_dir / "love"))
        self.run_script("uninstall")
        self.assertNotIn(BEGIN, config.read_text())

    def test_xdg_data_home(self):
        self.env["XDG_DATA_HOME"] = str(self.home / "custom data")
        self.run_script("install")
        installed = Path(self.env["XDG_DATA_HOME"]) / "lovelaojicli/bin/joy"
        self.assertTrue(installed.is_file())
        self.run_script("uninstall")
        self.assertFalse(installed.exists())

    def test_chinese_menu_and_invalid_arguments(self):
        self.env["LANG"] = "zh_CN.UTF-8"
        self.assertIn("编译并安装", self.run_script(answer="0\n").stdout)
        self.assertIn("用法", self.run_script("--help").stdout)
        self.run_script("unknown", ok=False)
        self.run_script("install", "extra", ok=False)
        self.run_script(answer="9\n", ok=False)

    def test_unsupported_shell_and_relative_data_home(self):
        self.env["SHELL"] = "/bin/tcsh"
        self.run_script("install", ok=False)
        self.env["XDG_DATA_HOME"] = "relative"
        self.run_script("install", ok=False)
        self.assertEqual(list(self.home.iterdir()), [])


if __name__ == "__main__":
    unittest.main()
