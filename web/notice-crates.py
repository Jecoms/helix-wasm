#!/usr/bin/env python3
"""Regenerate the Rust-crate table in web/NOTICE.md from the real dependency
graph of the shipped wasm.

The notice travels with what this project distributes (the release tarball and
the deployed page), so the crate list has to be true about the artifact rather
than about the workspace: only the crates whose compiled code ends up inside
`helix_web_bg.wasm`.

What counts, and why:

  * root         `helix-web` built for `wasm32-unknown-unknown` — the crate
                 wasm-pack builds. `--filter-platform` drops the deps that only
                 exist on other targets (there are plenty: helix pulls in a
                 unix terminal stack natively).
  * edges        normal dependencies only. `dev-dependencies` never reach a
                 non-test build, and `build-dependencies` run on the build host
                 and link nothing into the wasm.
  * proc-macros  pruned, along with everything reachable only through one
                 (`syn`, `quote`, `proc-macro2`, ...): a proc-macro is a host
                 dylib the compiler loads, so no byte of it is in the artifact.
                 Crates like `cc` and `unicode-ident` still appear, because
                 something also depends on them as an ordinary dependency.

License and copyright are read from the package itself, never guessed:

  * the SPDX expression is the `license` field the package's own manifest
    declares. Where a package offers a choice (`MIT OR Apache-2.0`), the whole
    expression is reproduced — electing one on the recipient's behalf is not
    this file's job.
  * the copyright lines are the `Copyright ...` lines of the package's own
    license files: from the `.crate` archive in the cargo registry cache for
    registry packages, and from the package directory for the path packages in
    this repository. Only lines starting at column zero count, which is what
    keeps the indented `copyright` mentions inside a license body out; the
    Apache-2.0 appendix template is filtered by name. A package that ships no
    copyright line gets `—`, which is a statement about the package, not a gap
    in this script.

The path packages under `stubs/` are the interesting ones: four are vendored
third-party code and declare their upstream license, while `stubs/home`,
`stubs/libloading` and `stubs/which` are this repository's own from-scratch
shims — they contain no upstream code and declare the repository's `MPL-2.0`.

Every path package therefore declares a license today, and the
`MPL-2.0 (repository)` rendering below is the fallback for one that does not:
a new stub added without a `license` field still gets the repository's terms
rather than a blank cell.

Usage (from anywhere; paths are resolved relative to this file):

    python3 web/notice-crates.py            # rewrite the generated block
    python3 web/notice-crates.py --check    # exit 1 if it is out of date

`--check` is what CI runs. Beyond staleness it enforces two things about the
licenses themselves, both of which need a human when they trip: that every
identifier in the tree is one this notice knows about, and that every crate can
be taken under terms whose text this notice actually carries — a crate offered
*only* under a license reproduced nowhere fails, however familiar the
identifier is.

Requires nothing but a stable toolchain and python3: registry packages must be
downloaded (`cargo fetch --locked`) so their `.crate` archives are in the cache.
"""

from __future__ import annotations

import argparse
import glob
import json
import os
import re
import subprocess
import sys
import tarfile

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
NOTICE = os.path.join(HERE, "NOTICE.md")

ROOT_PACKAGE = "helix-web"
TARGET = "wasm32-unknown-unknown"
# The terms of the repository's own `LICENSE`, which the path packages that
# declare no `license` of their own fall under.
REPOSITORY_LICENSE = "MPL-2.0"

BEGIN = "<!-- BEGIN GENERATED CRATE TABLE — regenerate with web/notice-crates.py -->"
END = "<!-- END GENERATED CRATE TABLE -->"

# Licenses whose text this notice actually carries, so that a recipient of the
# artifact has the terms in hand. Every crate in the table must be takeable
# under some combination of these — see `covered`.
REPRODUCED = {
    # Reproduced in full at the end of web/NOTICE.md.
    "MIT",
    "Apache-2.0",
    "Unicode-3.0",
    "Zlib",
    "BSD-3-Clause",
    # The repository's own terms: the root LICENSE, which ships beside this
    # notice in both distributed forms.
    "MPL-2.0",
    "MPL-2.0+",
}

# Licenses that appear in the tree but whose text this notice does not carry.
# Every crate offering one of these also offers something in REPRODUCED, which
# `covered` is what enforces — a crate offering one of them *alone* fails the
# check and has to be looked at by a human.
NOT_REPRODUCED = {
    "Unlicense",
    "CC0-1.0",
    "MIT-0",
}

KNOWN_LICENSES = REPRODUCED | NOT_REPRODUCED

LICENSE_FILE = re.compile(r"(?i)^(licen[cs]e|copying|notice)")
# The Apache-2.0 appendix is a fill-in-the-blanks copyright line. Crates that
# paste the whole license into one combined file carry it either untouched
# (`Copyright [yyyy] [name of copyright owner]`) or filled in with the brackets
# left in place (`Copyright [2015] [Dan Burkert]`, memmap2's LICENSE-APACHE).
# Neither is a copyright notice worth reproducing; the crate's real one is
# elsewhere in its own files. The rest of the Apache boilerplate never matches,
# because its `copyright` mentions are all indented and this only takes lines
# that start at column zero.
PLACEHOLDER = re.compile(r"\[yyyy\]|\[name of copyright owner\]|\[fullname\]|\[.+\] \[.+\]")


def run(*args: str) -> str:
    try:
        return subprocess.run(
            args, cwd=ROOT, check=True, capture_output=True, text=True
        ).stdout
    except subprocess.CalledProcessError as error:
        # Without this, cargo's own diagnostics are swallowed by
        # capture_output and CI reports a bare traceback. The likeliest
        # failure here is `--locked` refusing a lockfile that needs updating,
        # which is precisely the message worth showing.
        sys.exit(
            f"error: `{' '.join(args)}` failed with status {error.returncode}\n"
            + (error.stderr or "").rstrip()
        )


def copyright_lines(text: str) -> list[str]:
    """The `Copyright ...` lines of one license file, verbatim and in order."""
    out = []
    for line in text.splitlines():
        # Column zero only: the copyright-adjacent lines in a license *body*
        # ("...shall mean the copyright owner...") are all indented.
        if not line.startswith("Copyright"):
            continue
        line = line.strip().rstrip(".")
        if PLACEHOLDER.search(line) or len(line) > 150 or line in out:
            continue
        out.append(line)
    return out


def covered(expression: str) -> bool:
    """Can a recipient take this crate under terms this notice reproduces?

    A tiny SPDX evaluator: `A OR B` needs one side covered, `A AND B` needs
    both. Enough for the expressions crates.io actually carries, and it is the
    per-crate question — asking only whether each identifier is *known* would
    wave through a crate offered solely under a license whose text is missing.
    """
    tokens = re.findall(r"\(|\)|[^\s()]+", expression.replace("/", " OR "))
    position = 0

    def factor() -> bool:
        nonlocal position
        token = tokens[position]
        position += 1
        if token == "(":
            value = disjunction()
            position += 1  # ")"
            return value
        return token in REPRODUCED

    def conjunction() -> bool:
        value = factor()
        nonlocal position
        while position < len(tokens) and tokens[position] == "AND":
            position += 1
            value = factor() and value
        return value

    def disjunction() -> bool:
        value = conjunction()
        nonlocal position
        while position < len(tokens) and tokens[position] == "OR":
            position += 1
            value = conjunction() or value
        return value

    return disjunction()


def from_archive(name: str, version: str) -> list[str]:
    """Copyright lines from a registry package's `.crate` archive."""
    # Several registries can be configured at once (a sparse and a git index
    # for crates.io alone give two cache directories). Any archive with this
    # name and version will do: cargo verifies each against the registry
    # checksum, so whichever is found first holds identical bytes.
    for cache in glob.glob(os.path.expanduser("~/.cargo/registry/cache/*/")):
        path = os.path.join(cache, f"{name}-{version}.crate")
        if not os.path.exists(path):
            continue
        out: list[str] = []
        with tarfile.open(path) as tar:
            for member in sorted(tar.getmembers(), key=lambda m: m.name):
                parts = member.name.split("/")
                # Top level of the archive only: a nested license belongs to
                # vendored code, which the hand-written sections cover.
                if len(parts) != 2 or not LICENSE_FILE.match(parts[1]):
                    continue
                handle = tar.extractfile(member)
                if handle is None:
                    continue
                for line in copyright_lines(handle.read().decode("utf-8", "replace")):
                    if line not in out:
                        out.append(line)
        return out
    sys.exit(
        f"error: no .crate archive for {name} {version} in the registry cache.\n"
        f"       run `cargo fetch --locked` first."
    )


def from_directory(directory: str) -> list[str]:
    """Copyright lines from a path package's own license files."""
    out: list[str] = []
    for entry in sorted(os.listdir(directory)):
        path = os.path.join(directory, entry)
        if not LICENSE_FILE.match(entry) or not os.path.isfile(path):
            continue
        # Skip this file itself, and only this file: it lives in the helix-web
        # package directory and matches `NOTICE`, so the license texts it
        # reproduces would come back as helix-web's own copyright. A NOTICE
        # shipped by a vendored crate under stubs/ is a real notice and stays.
        if os.path.abspath(path) == NOTICE:
            continue
        with open(path, encoding="utf-8", errors="replace") as handle:
            for line in copyright_lines(handle.read()):
                if line not in out:
                    out.append(line)
    return out


TREE_LINE = re.compile(r"^(?P<name>[^\s]+) v(?P<version>[^\s]+)(?: .*)?$")


def linked_packages() -> list[dict]:
    """The packages linked into the wasm, as cargo resolves them.

    Membership comes from `cargo tree` rather than from a walk of
    `cargo metadata`'s resolve graph: metadata resolves features across the
    whole workspace at once, which turns on `tokio/net` (and so pulls in `mio`)
    for a build that never enables it. `cargo tree -p helix-web` resolves for
    exactly the package wasm-pack builds, which is the artifact this notice
    describes. Metadata is then used only as a package database, for the
    `license`, `source` and `manifest_path` of each package the lockfile holds.
    """
    tree = run(
        "cargo",
        "tree",
        "--locked",
        "--package",
        ROOT_PACKAGE,
        "--target",
        TARGET,
        # `no-proc-macro` prunes proc-macro packages and everything reachable
        # only through one; `normal` drops the dev and build edges.
        "--edges",
        "normal,no-proc-macro",
        "--prefix",
        "none",
    )
    wanted: set[tuple[str, str]] = set()
    for line in tree.splitlines():
        # `(*)` marks a subtree cargo already printed in full.
        line = line.strip().removesuffix("(*)").strip()
        if not line:
            continue
        match = TREE_LINE.match(line)
        if not match:
            sys.exit(f"error: cannot parse a cargo tree line: {line!r}")
        wanted.add((match["name"], match["version"]))
    if not any(name == ROOT_PACKAGE for name, _ in wanted):
        sys.exit(f"error: cargo tree output does not contain {ROOT_PACKAGE}")

    metadata = json.loads(run("cargo", "metadata", "--format-version", "1", "--locked"))
    packages = {(p["name"], p["version"]): p for p in metadata["packages"]}
    missing = sorted(k for k in wanted if k not in packages)
    if missing:
        sys.exit(f"error: not in cargo metadata: {missing}")

    return sorted((packages[k] for k in wanted), key=lambda p: (p["name"], p["version"]))


def escape(cell: str) -> str:
    return cell.replace("|", r"\|").replace("<", r"\<").replace(">", r"\>")


def render(packages: list[dict]) -> str:
    rows = [
        "| Crate | Version | License | Copyright | Source |",
        "| --- | --- | --- | --- | --- |",
    ]
    unknown: dict[str, set[str]] = {}
    uncovered: list[str] = []
    for pkg in packages:
        name, version = pkg["name"], pkg["version"]
        source = pkg.get("source")
        declared = pkg.get("license")
        if source is None:
            directory = os.path.dirname(pkg["manifest_path"])
            relative = os.path.relpath(directory, ROOT)
            origin = "repository root" if relative == "." else f"`{relative}`"
            holders = from_directory(directory)
            # A path package that declares nothing is this repository's own
            # code, governed by the root LICENSE.
            shown = declared or f"{REPOSITORY_LICENSE} (repository)"
            effective = declared or REPOSITORY_LICENSE
        elif "crates.io" in source:
            origin = f"[crates.io](https://crates.io/crates/{name}/{version})"
            holders = from_archive(name, version)
            if not declared:
                sys.exit(f"error: {name} {version} declares no license")
            shown = effective = declared
        else:
            sys.exit(f"error: {name} {version} comes from an unhandled source: {source}")

        for identifier in re.split(r"[()/]|\bOR\b|\bAND\b|\bWITH\b", effective):
            identifier = identifier.strip()
            if identifier and identifier not in KNOWN_LICENSES:
                unknown.setdefault(identifier, set()).add(f"{name} {version}")
        if not covered(effective):
            uncovered.append(f"{name} {version} ({effective})")

        rows.append(
            "| `{}` | {} | {} | {} | {} |".format(
                name,
                version,
                escape(shown),
                escape("; ".join(holders)) if holders else "—",
                origin,
            )
        )

    if unknown:
        sys.exit(
            "error: license identifier(s) this notice knows nothing about:\n"
            + "".join(
                f"       {identifier} — {', '.join(sorted(crates))}\n"
                for identifier, crates in sorted(unknown.items())
            )
            + "       a dependency arrived under terms the notice has not\n"
            "       accounted for. Decide whether its text has to be reproduced\n"
            "       in NOTICE.md, then add the identifier to REPRODUCED or\n"
            "       NOT_REPRODUCED."
        )
    if uncovered:
        sys.exit(
            "error: crate(s) whose only terms are a license this notice does not\n"
            "       reproduce, so a recipient of the artifact would not have\n"
            "       them in hand:\n"
            + "".join(f"       {entry}\n" for entry in uncovered)
            + "       reproduce the license text in NOTICE.md and move its\n"
            "       identifier into REPRODUCED."
        )

    return "\n".join(rows)


def splice(notice: str, table: str) -> str:
    start, end = notice.find(BEGIN), notice.find(END)
    if start < 0 or end < 0:
        sys.exit(f"error: {NOTICE} is missing the generated-block markers")
    return notice[: start + len(BEGIN)] + "\n\n" + table + "\n\n" + notice[end:]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="exit 1 if NOTICE.md is out of date instead of rewriting it",
    )
    args = parser.parse_args()

    with open(NOTICE, encoding="utf-8") as handle:
        current = handle.read()
    updated = splice(current, render(linked_packages()))

    if args.check:
        if current != updated:
            print(
                "web/NOTICE.md is out of date with the wasm32 dependency graph.\n"
                "Run `python3 web/notice-crates.py` and commit the result.",
                file=sys.stderr,
            )
            return 1
        print("web/NOTICE.md matches the wasm32 dependency graph.")
        return 0

    if current != updated:
        with open(NOTICE, "w", encoding="utf-8") as handle:
            handle.write(updated)
        print(f"updated {NOTICE}")
    else:
        print(f"{NOTICE} already up to date")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
