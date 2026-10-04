"""Isolated installer tests; no real HOME edits, compilation or network access."""

import os
import hashlib
import io
import re
import tarfile
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

SCRIPT = Path(__file__).resolve().parents[1] / "install.sh"
BEGIN = "# >>> LoveLaoJiCLI PATH >>>"
END = "# <<< LoveLaoJiCLI PATH <<<"


def _default_version():
    """Track the installer's pinned default so bumps need no test edits."""
    match = re.search(
        r"LOVELAOJI_VERSION:-(v[0-9]+\.[0-9]+\.[0-9]+)", SCRIPT.read_text()
    )
    if match is None:
        raise SystemExit("install.sh: cannot find the pinned LOVELAOJI_VERSION default")
    return match.group(1)


DEFAULT_VERSION = _default_version()


class InstallerFixture(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="lovelaojicli-test-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.home = self.root / "home with 'quote and $dollar"
        self.home.mkdir()
        self.project = self.root / "project with spaces"
        self.project.mkdir()
        shutil.copy2(SCRIPT, self.project / "install.sh")
        (self.project / "Cargo.toml").write_text("# mock checkout\n")
        self.fake_bin = self.root / "fake-bin"
        self.fake_bin.mkdir()
        self.executable("rustc", "#!/bin/sh\nprintf 'host: test-host\\n'\n")
        self.executable(
            "cargo",
            """#!/usr/bin/env bash
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
for tool in love happiness joy patience sprinkle later enough stuck duck one afk goodnight proof poke; do
    printf '#!/bin/sh\\nprintf "%s 0.1.0\\\\n"\\n' "$tool" > "$target/$host/release/$tool"
    chmod +x "$target/$host/release/$tool"
done
""",
        )
        self.env = os.environ.copy()
        for key in [
            "LC_ALL",
            "LC_MESSAGES",
            "LANGUAGE",
            "XDG_DATA_HOME",
            "XDG_CONFIG_HOME",
            "ZDOTDIR",
            "BASH_ENV",
            "ENV",
            "LOVELAOJI_VERSION",
        ]:
            self.env.pop(key, None)
        self.env.update(
            HOME=str(self.home),
            SHELL="/bin/bash",
            LANG="C",
            PATH=f"{self.fake_bin}:/usr/bin:/bin",
        )
        self.install_dir = self.home / ".local/share/lovelaojicli"
        self.bin_dir = self.install_dir / "bin"

    def executable(self, name, content):
        path = self.fake_bin / name
        path.write_text(content)
        path.chmod(0o755)

    def run_script(self, *args, answer="y\n", ok=True):
        result = subprocess.run(
            ["bash", str(self.project / "install.sh"), *args],
            input=answer,
            text=True,
            encoding="utf-8",
            errors="replace",
            capture_output=True,
            cwd=self.root,
            env=self.env,
            timeout=20,
        )
        if ok:
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        else:
            self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
        return result


class LocalInstallerTests(InstallerFixture):
    def test_install_idempotent_path_and_safe_uninstall(self):
        bashrc = self.home / ".bashrc"
        original = "# user configuration\nexport KEEP_ME=yes\n"
        bashrc.write_text(original)
        bashrc.chmod(0o640)
        self.run_script(answer="3\ny\n")
        first = bashrc.read_text()
        self.run_script("install-local")
        self.assertEqual(bashrc.read_text(), first)
        self.assertEqual(first.count(BEGIN), 1)
        self.assertEqual(bashrc.stat().st_mode & 0o777, 0o640)
        self.assertIn(BEGIN, (self.home / ".profile").read_text())
        result = subprocess.run(
            [
                "bash",
                "--noprofile",
                "--norc",
                "-c",
                '. "$HOME/.bashrc"; . "$HOME/.bashrc"; command -v love; love; printf "%s\\n" "$PATH"',
            ],
            env=self.env,
            text=True,
            capture_output=True,
            check=True,
        )
        self.assertEqual(result.stdout.splitlines()[0], str(self.bin_dir / "love"))
        self.assertIn("love 0.1.0", result.stdout)
        self.assertEqual(
            result.stdout.splitlines()[-1].split(":").count(str(self.bin_dir)), 1
        )
        for tool in [
            "love",
            "happiness",
            "joy",
            "patience",
            "sprinkle",
            "later",
            "enough",
            "stuck",
            "duck",
            "one",
            "afk",
            "goodnight",
            "proof",
            "poke",
        ]:
            self.assertTrue((self.bin_dir / tool).is_file())
        saved_state = self.home / ".local/state/lovelaojicli/keep-card"
        saved_state.parent.mkdir(parents=True)
        saved_state.write_text("user context")
        unrelated = self.bin_dir / "keep-me"
        unrelated.write_text("untouched")
        self.run_script("uninstall")
        self.assertEqual(bashrc.read_text(), original)
        self.assertEqual(unrelated.read_text(), "untouched")
        self.assertEqual(saved_state.read_text(), "user context")
        for tool in [
            "love",
            "happiness",
            "joy",
            "patience",
            "sprinkle",
            "later",
            "enough",
            "stuck",
            "duck",
            "one",
            "afk",
            "goodnight",
            "proof",
            "poke",
        ]:
            self.assertFalse((self.bin_dir / tool).exists())
        self.run_script("uninstall")

    def test_cancel_and_eof_do_not_install(self):
        for args, answer in [
            ((), "0\n"),
            ((), ""),
            (("install-local",), "n\n"),
            (("install-local",), ""),
        ]:
            self.run_script(*args, answer=answer)
            self.assertFalse(self.install_dir.exists())
            self.assertFalse((self.home / ".bashrc").exists())

    def test_build_failure_does_not_modify_home(self):
        self.env["FAIL_BUILD"] = "1"
        self.run_script("install-local", ok=False)
        self.assertEqual(list(self.home.iterdir()), [])

    def test_failed_upgrade_preserves_installation(self):
        self.run_script("install-local")
        before = (self.bin_dir / "love").read_bytes()
        self.env["FAIL_BUILD"] = "1"
        self.run_script("install-local", ok=False)
        self.assertEqual((self.bin_dir / "love").read_bytes(), before)

    def test_existing_unmanaged_install_is_not_overwritten(self):
        self.install_dir.mkdir(parents=True)
        sentinel = self.install_dir / "mine"
        sentinel.write_text("keep")
        self.run_script("install-local", ok=False)
        self.assertEqual(sentinel.read_text(), "keep")

    def test_malformed_block_is_not_modified(self):
        bashrc = self.home / ".bashrc"
        original = f"# keep\n{BEGIN}\nunfinished\n"
        bashrc.write_text(original)
        self.run_script("install-local", ok=False)
        self.assertEqual(bashrc.read_text(), original)
        self.assertFalse(self.install_dir.exists())

    def test_binary_directory_is_not_overwritten(self):
        self.run_script("install-local")
        binary = self.bin_dir / "love"
        binary.unlink()
        binary.mkdir()
        self.run_script("install-local", ok=False)
        self.assertEqual(list(binary.iterdir()), [])

    def test_bash_login_profile_selection(self):
        for name in [".bash_profile", ".bash_login"]:
            with self.subTest(name=name):
                profile = self.home / name
                profile.write_text("# login\n")
                self.run_script("install-local")
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
        self.run_script("install-local")
        self.assertTrue(link.is_symlink())
        self.assertIn(BEGIN, target.read_text())
        self.run_script("uninstall")
        self.assertEqual(target.read_text(), "# dotfiles\n")

    def test_zsh_custom_location_and_uninstall_after_shell_change(self):
        self.env.update(SHELL="/bin/zsh", ZDOTDIR=str(self.home / "zsh configs"))
        self.run_script("install-local")
        files = [Path(self.env["ZDOTDIR"]) / name for name in [".zshrc", ".zprofile"]]
        for file in files:
            self.assertIn(BEGIN, file.read_text())
        self.env["SHELL"] = "/bin/bash"
        self.run_script("uninstall")
        for file in files:
            self.assertNotIn(BEGIN, file.read_text())

    def test_fish_configuration(self):
        self.env.update(
            SHELL="/usr/bin/fish", XDG_CONFIG_HOME=str(self.home / "custom config")
        )
        self.run_script("install-local")
        config = Path(self.env["XDG_CONFIG_HOME"]) / "fish/conf.d/lovelaojicli.fish"
        self.assertIn("set -gx PATH", config.read_text())
        if shutil.which("fish"):
            result = subprocess.run(
                [
                    shutil.which("fish"),
                    "--no-config",
                    "-c",
                    'source "$XDG_CONFIG_HOME/fish/conf.d/lovelaojicli.fish"; source "$XDG_CONFIG_HOME/fish/conf.d/lovelaojicli.fish"; command -s love; love',
                ],
                env=self.env,
                text=True,
                capture_output=True,
                check=True,
            )
            self.assertEqual(result.stdout.splitlines()[0], str(self.bin_dir / "love"))
        self.run_script("uninstall")
        self.assertNotIn(BEGIN, config.read_text())

    def test_xdg_data_home(self):
        self.env["XDG_DATA_HOME"] = str(self.home / "custom data")
        self.run_script("install-local")
        installed = Path(self.env["XDG_DATA_HOME"]) / "lovelaojicli/bin/joy"
        self.assertTrue(installed.is_file())
        self.run_script("uninstall")
        self.assertFalse(installed.exists())

    def test_chinese_menu_and_invalid_arguments(self):
        self.env["LANG"] = "zh_CN.UTF-8"
        self.assertIn("编译并安装", self.run_script(answer="0\n").stdout)
        self.assertIn("用法", self.run_script("--help").stdout)
        self.run_script("unknown", ok=False)
        self.run_script("install-local", "extra", ok=False)
        self.run_script(answer="9\n", ok=False)

    def test_unsupported_shell_and_relative_data_home(self):
        self.env["SHELL"] = "/bin/tcsh"
        self.run_script("install-local", ok=False)
        self.env["XDG_DATA_HOME"] = "relative"
        self.run_script("install-local", ok=False)
        self.assertEqual(list(self.home.iterdir()), [])


TOOLS = "love happiness joy patience sprinkle later enough stuck duck one afk goodnight proof poke".split()


class ReleaseInstallerTests(InstallerFixture):
    def setUp(self):
        super().setUp()
        self.assets = self.root / "assets"
        self.assets.mkdir()
        self.env.update(
            ASSET_DIR=str(self.assets),
            DOWNLOAD_LOG=str(self.root / "downloads"),
            MOCK_OS="Linux",
            MOCK_ARCH="x86_64",
        )
        self.executable(
            "uname",
            "#!/bin/sh\ncase $1 in -s) echo $MOCK_OS;; -m) echo $MOCK_ARCH;; esac\n",
        )
        # Any attempt to compile in release mode fails the test.
        self.executable("cargo", "#!/bin/sh\nexit 99\n")
        self.executable("rustc", "#!/bin/sh\nexit 99\n")
        self.executable(
            "curl",
            """#!/usr/bin/env bash
set -eu
[[ ${FAIL_DOWNLOAD:-0} == 0 ]] || exit 22
while [[ $# -gt 0 ]]; do
    case "$1" in
        https://*) url=$1 ;;
        --output) output=$2; shift ;;
    esac
    shift
done
printf '%s\n' "$url" >> "$DOWNLOAD_LOG"
[[ $url == https://github.com/Dailiduzhou/LoveLaoJiCLI/releases/download/* ]]
cp "$ASSET_DIR/${url##*/}" "$output"
""",
        )
        self.make_archive()

    def make_archive(
        self, target="x86_64-unknown-linux-gnu", version=DEFAULT_VERSION, variant=None
    ):
        archive = self.assets / f"lovelaojicli-{version}-{target}.tar.gz"
        with tarfile.open(archive, "w:gz") as tar:
            for tool in TOOLS:
                if variant == "missing" and tool == "poke":
                    continue
                data = f'#!/bin/sh\nprintf "{tool} {version[1:]}\\n"\n'.encode()
                if tool == "poke" and variant == "wrong-version":
                    data = b'#!/bin/sh\necho "poke 0.0.0"\n'
                if tool == "poke" and variant == "cannot-run":
                    data += b"exit 1\n"
                member = tarfile.TarInfo(tool)
                member.size = len(data)
                member.mode = 0o755
                if variant == "symlink" and tool == "poke":
                    member.type = tarfile.SYMTYPE
                    member.linkname = str(self.home / "escaped")
                    member.size = 0
                    tar.addfile(member)
                else:
                    tar.addfile(member, io.BytesIO(data))
            if variant in ("traversal", "duplicate"):
                member = tarfile.TarInfo(
                    "../escaped" if variant == "traversal" else "love"
                )
                tar.addfile(member)
        checksum = hashlib.sha256(archive.read_bytes()).hexdigest()
        archive.with_name(archive.name + ".sha256").write_text(
            f"{checksum}  {archive.name}\n"
        )
        return archive

    def test_platform_mapping_and_no_rust_required(self):
        for system, arch, target in [
            ("Linux", "x86_64", "x86_64-unknown-linux-gnu"),
            ("Linux", "aarch64", "aarch64-unknown-linux-gnu"),
            ("Linux", "arm64", "aarch64-unknown-linux-gnu"),
            ("Darwin", "x86_64", "x86_64-apple-darwin"),
            ("Darwin", "arm64", "aarch64-apple-darwin"),
        ]:
            with self.subTest(system=system, arch=arch):
                self.make_archive(target)
                self.env.update(MOCK_OS=system, MOCK_ARCH=arch)
                self.run_script("install")
                self.assertIn(
                    f"{DEFAULT_VERSION}-{target}.tar.gz", (self.root / "downloads").read_text()
                )
                for tool in TOOLS:
                    self.assertTrue(os.access(self.bin_dir / tool, os.X_OK))
                self.run_script("uninstall")

    def test_default_menu_install_and_idempotence(self):
        self.run_script(answer="1\ny\n")
        original = (self.home / ".bashrc").read_text()
        self.run_script("install")
        self.assertEqual((self.home / ".bashrc").read_text(), original)
        self.run_script("uninstall")
        self.assertFalse(self.bin_dir.exists())

    def test_explicit_version(self):
        self.env["LOVELAOJI_VERSION"] = "v0.4.2"
        self.make_archive(version="v0.4.2")
        self.run_script("install")
        self.assertIn("/v0.4.2/", (self.root / "downloads").read_text())

    def test_download_failure_and_failed_upgrade(self):
        self.env["FAIL_DOWNLOAD"] = "1"
        self.run_script("install", ok=False)
        self.assertEqual(list(self.home.iterdir()), [])
        self.env.pop("FAIL_DOWNLOAD")
        self.run_script("install")
        before = (self.bin_dir / "love").read_bytes()
        rc = (self.home / ".bashrc").read_bytes()
        self.env["FAIL_DOWNLOAD"] = "1"
        self.run_script("install", ok=False)
        self.assertEqual((self.bin_dir / "love").read_bytes(), before)
        self.assertEqual((self.home / ".bashrc").read_bytes(), rc)

    def test_checksum_mismatch(self):
        archive = self.make_archive()
        archive.write_bytes(archive.read_bytes() + b"corrupt")
        self.assertIn("checksum", self.run_script("install", ok=False).stderr)
        self.assertEqual(list(self.home.iterdir()), [])

    def test_unsafe_or_incomplete_archives(self):
        for variant in ["missing", "symlink", "traversal", "duplicate"]:
            with self.subTest(variant=variant):
                self.make_archive(variant=variant)
                self.run_script("install", ok=False)
                self.assertEqual(list(self.home.iterdir()), [])
                self.assertFalse((self.root / "escaped").exists())

    def test_wrong_version_or_failed_execution_preserves_home(self):
        for variant in ["wrong-version", "cannot-run"]:
            with self.subTest(variant=variant):
                self.make_archive(variant=variant)
                self.run_script("install", ok=False)
                self.assertEqual(list(self.home.iterdir()), [])

    def test_checksum_manifest_must_name_exact_asset(self):
        archive = self.make_archive()
        manifest = archive.with_name(archive.name + ".sha256")
        manifest.write_text(manifest.read_text().replace(archive.name, "../other"))
        self.run_script("install", ok=False)
        self.assertEqual(list(self.home.iterdir()), [])

    def test_unsupported_platform_and_invalid_version(self):
        self.env["MOCK_OS"] = "FreeBSD"
        self.run_script("install", ok=False)
        self.env["MOCK_OS"] = "Linux"
        self.env["LOVELAOJI_VERSION"] = "../../invalid"
        self.run_script("install", ok=False)
        self.assertFalse((self.root / "downloads").exists())
        self.assertEqual(list(self.home.iterdir()), [])

    def test_cancel_never_downloads(self):
        for answer in ["", "n\n"]:
            self.run_script("install", answer=answer)
        self.assertFalse((self.root / "downloads").exists())
        self.assertEqual(list(self.home.iterdir()), [])


if __name__ == "__main__":
    unittest.main()
