#!/usr/bin/env python3
"""Show or set Seula's version in every file that carries it (`just version`).

    just version                  print the version, and say if the files disagree
    just version 0.2.0-alpha.3    set it everywhere

The seven places: the daemon's Cargo.toml and Cargo.lock, the frontend's package.json and
package-lock.json (two entries), and the shell's Cargo.toml, Cargo.lock and tauri.conf.json.
Setting refuses a version that is not newer than the current one, or that already has a git
tag (a version number is never reused), and refuses to start while the files disagree. After
the edit it asks cargo to check both lock files (`--locked`) and puts every file back if
either complains. It does not commit, tag or push.
"""

import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# Each place is (path, how to read the version, regex that matches the value to replace).
# The patterns capture the text before the version (group 1), the version (2) and the text
# after it (3), so only the version changes and line endings and layout are left alone.
FILES = [
    ("Cargo.toml", r'(?m)^(version = ")([^"]+)(")'),
    ("Cargo.lock", r'(name = "seula"\r?\nversion = ")([^"]+)(")'),
    ("web/solid/package.json", r'(?m)^(  "version": ")([^"]+)(")'),
    ("web/solid/package-lock.json", r'(?m)^(  "version": ")([^"]+)(")'),
    ("web/solid/package-lock.json", r'(?m)^(      "version": ")([^"]+)(")'),
    ("web/solid/src-tauri/Cargo.toml", r'(?m)^(version = ")([^"]+)(")'),
    ("web/solid/src-tauri/Cargo.lock", r'(name = "seula-shell-solid"\r?\nversion = ")([^"]+)(")'),
    ("web/solid/src-tauri/tauri.conf.json", r'(?m)^(  "version": ")([^"]+)(")'),
]

# The two Rust projects whose lock files must still agree with their manifests afterwards.
CARGO_DIRS = [ROOT, ROOT / "web/solid/src-tauri"]

SEMVER = re.compile(r"^(\d+)\.(\d+)\.(\d+)(?:-([0-9A-Za-z.-]+))?$")


def read(path: str) -> str:
    return (ROOT / path).read_bytes().decode("utf-8")


def found(path: str, pattern: str) -> str | None:
    m = re.search(pattern, read(path))
    return m.group(2) if m else None


def key(version: str):
    """Sort key following semver precedence: a pre-release is older than its release."""
    major, minor, patch, pre = SEMVER.match(version).groups()
    ids = [] if pre is None else [(0, int(i), "") if i.isdigit() else (1, 0, i) for i in pre.split(".")]
    # No pre-release sorts after any pre-release of the same x.y.z.
    return (int(major), int(minor), int(patch), pre is None, ids)


def current_versions():
    return [(path, found(path, pattern)) for path, pattern in FILES]


def report(versions) -> str | None:
    """Print each file's version; return the common one, or None if they disagree."""
    width = max(len(p) for p, _ in versions)
    for path, v in versions:
        print(f"  {path:<{width}}  {v if v is not None else '(not found)'}")
    values = {v for _, v in versions}
    if len(values) == 1 and None not in values:
        return values.pop()
    return None


def cargo_locked_ok() -> bool:
    for d in CARGO_DIRS:
        r = subprocess.run(
            ["cargo", "metadata", "--no-deps", "--format-version", "1", "--locked"],
            cwd=d, capture_output=True, text=True,
        )
        if r.returncode != 0:
            print(f"cargo metadata --locked failed in {d.relative_to(ROOT) if d != ROOT else '.'}:")
            print(r.stderr.strip())
            return False
    return True


def main(argv: list[str]) -> int:
    if len(argv) > 1:
        print("usage: just version [NEW_VERSION]")
        return 2

    versions = current_versions()
    common = report(versions)

    if not argv:
        if common is None:
            print("\nThe files disagree. Run `just version <the right one>` once they match, or fix them by hand.")
            return 1
        print(f"\nVersion {common}")
        return 0

    new = argv[0]
    if not SEMVER.match(new):
        print(f"\n'{new}' is not a version like 0.2.0-alpha.3 or 1.0.0")
        return 2
    if common is None:
        print("\nThe files disagree, so there is no single current version to move on from. Make them match first.")
        return 1
    if not SEMVER.match(common):
        print(f"\nThe current version '{common}' is not a plain semver version; edit the files by hand.")
        return 1
    if new == common:
        print(f"\nAlready at {new}.")
        return 1
    if key(new) < key(common):
        print(f"\n{new} is older than {common}. A version only goes forward.")
        return 1
    tags = subprocess.run(["git", "tag", "--list", new], cwd=ROOT, capture_output=True, text=True).stdout.split()
    if new in tags:
        print(f"\nThe tag {new} already exists. A version number is never reused.")
        return 1

    # Edit every place, keeping the original bytes so a failed check can put them back.
    originals = {}

    def restore():
        for p, t in originals.items():
            (ROOT / p).write_bytes(t.encode("utf-8"))

    for path, pattern in FILES:
        originals.setdefault(path, read(path))
        # Read again each time: package-lock.json has two entries, and the second edit
        # must build on the first.
        edited, n = re.subn(pattern, lambda m: m.group(1) + new + m.group(3), read(path), count=1)
        if n != 1:
            print(f"\nCould not find the version in {path}; putting everything back.")
            restore()
            return 1
        (ROOT / path).write_bytes(edited.encode("utf-8"))

    # package-lock.json and the manifests must still be valid JSON with the new value.
    try:
        for p in ("web/solid/package.json", "web/solid/src-tauri/tauri.conf.json"):
            assert json.loads(read(p))["version"] == new
        lock = json.loads(read("web/solid/package-lock.json"))
        assert lock["version"] == new and lock["packages"][""]["version"] == new
    except (AssertionError, KeyError, ValueError) as e:
        print(f"\nA JSON file did not come out right ({e!r}); putting everything back.")
        restore()
        return 1

    if not cargo_locked_ok():
        print("\nA lock file disagrees with its manifest; putting everything back.")
        restore()
        return 1

    print(f"\nVersion {common} -> {new}\n")
    report(current_versions())
    print("\nNothing is committed or tagged. Next: update CHANGELOG.md, commit, `just release`, then tag.")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
