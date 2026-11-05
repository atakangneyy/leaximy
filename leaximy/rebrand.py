#!/usr/bin/env python3
"""Renames a Spotifast checkout to Leaximy.

Run it from the repository root after pulling a new upstream version:

    python3 leaximy/rebrand.py

It is safe to run more than once. It changes:

* the visible name "Spotifast" to "Leaximy" (code, translations, build script),
* runtime identities ("spotifast" -> "leaximy"): settings/cache folders,
  credential-store service, D-Bus/MPRIS names, window app id, log file,
  so Leaximy never reads, overwrites or signs out a real Spotifast install,
* the old "fastpotify" migration identities to a name nothing uses, so
  Leaximy never moves or deletes an old Fastpotify profile or sign-in,
* the Cargo package and command name to "leaximy".

It leaves alone: the library crate path (`spotifast::`), links to
spotifast.rocks and github.com/crmne/spotifast (credit and docs), and the
names of files that ship inside the repository.
"""

from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parent.parent

# Lowercase "spotifast" becomes "leaximy" unless it is part of a path,
# a word, a crate path, a URL or a file name in the repository.
LOWER = re.compile(
    r"(?<![/\w-])spotifast"
    r"(?!::|\.rocks|:liked|\.json\.tpl|-theme\"|\.wsz|\.ico|\.rs|\.svg|-portable|-installer)"
)


def rebrand_text(text: str) -> str:
    text = text.replace("Spotifast", "Leaximy")
    text = LOWER.sub("leaximy", text)
    # Cargo names a test's path to the command after the binary.
    text = text.replace("CARGO_BIN_EXE_spotifast", "CARGO_BIN_EXE_leaximy")
    # Upstream's former name: point its migration at a profile, credential
    # service and D-Bus name that never exist.
    text = text.replace("Fastpotify", "Leaximy_Legacy")
    text = re.sub(r"(?<![/\w-])fastpotify(?![/\w]*\.(?:txt|rs))", "leaximy_legacy", text)
    return text


def rewrite(path: Path, transform) -> bool:
    before = path.read_text(encoding="utf-8")
    after = transform(before)
    if after != before:
        path.write_text(after, encoding="utf-8")
        return True
    return False


def main() -> int:
    changed = []
    sources = [
        *ROOT.glob("src/**/*.rs"),
        *ROOT.glob("tests/*.rs"),
        *ROOT.glob("examples/*.rs"),
        *ROOT.glob("assets/i18n/*.po"),
        ROOT / "assets/i18n/spotifast.pot",
        ROOT / "build.rs",
    ]
    for path in sources:
        if path.exists() and rewrite(path, rebrand_text):
            changed.append(path)

    # The single-instance guard and the command-line control protocol still
    # carry the old name upstream, but they are the running app's identity,
    # not a migration: give them the plain new name.
    instance = ROOT / "src/single_instance.rs"
    if rewrite(instance, lambda text: text.replace("leaximy_legacy", "leaximy")):
        changed.append(instance)

    # The compatibility command and the tests of upstream's rename and
    # updater do not apply to a fork.
    for stale in ["src/main.rs", "tests/branding.rs", "tests/update_launch.rs"]:
        path = ROOT / stale
        if path.exists():
            path.unlink()
            changed.append(path)

    def cargo_toml(text: str) -> str:
        text = re.sub(r'(?m)^name = "spotifast"\n(default-run = )"spotifast"',
                      r'name = "leaximy"\n\1"leaximy"', text)
        text = text.replace('description = "A native Spotify client"',
                            'description = "Leaximy, a native Spotify client based on Spotifast"')
        text = re.sub(r'(?m)^\[\[bin\]\]\nname = "spotifast"\n', '[[bin]]\nname = "leaximy"\n', text)
        text = re.sub(
            r'\n# Compatibility command for scripts and the updater in earlier releases\.\n'
            r'\[\[bin\]\]\nname = "fastpotify"\npath = "src/main\.rs"\n',
            "\n", text)
        return text

    def cargo_lock(text: str) -> str:
        return re.sub(r'(?m)^name = "spotifast"$', 'name = "leaximy"', text)

    for name, transform in [("Cargo.toml", cargo_toml), ("Cargo.lock", cargo_lock)]:
        if rewrite(ROOT / name, transform):
            changed.append(ROOT / name)

    # A Linux launcher matching the new app id.
    desktop = ROOT / "packaging/applications/spotifast.desktop"
    leaximy_desktop = ROOT / "packaging/applications/leaximy.desktop"
    if desktop.exists():
        text = rebrand_text(desktop.read_text(encoding="utf-8"))
        if not leaximy_desktop.exists() or leaximy_desktop.read_text(encoding="utf-8") != text:
            leaximy_desktop.write_text(text, encoding="utf-8")
            changed.append(leaximy_desktop)

    for path in changed:
        print(path.relative_to(ROOT))
    print(f"{len(changed)} file(s) changed.", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
