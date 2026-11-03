"""Validate and finish a draft release; all remote operations use the native gh CLI."""

import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tomllib


def run(*args):
    return subprocess.check_output(args, text=True).strip()


def notes(version, content):
    # Accept inherited Keep a Changelog entries and release-please's H2/H3 entries.
    headings = list(re.finditer(r"^#{2,3} (?:\[)?v?(Unreleased|\d+\.\d+\.\d+)(?=[\]\s(]|$).*?$", content, re.M))
    for index, heading in enumerate(headings):
        if heading[1] == version:
            end = headings[index + 1].start() if index + 1 < len(headings) else len(content)
            result = content[heading.end():end].strip()
            if result:
                return result + "\n"
    raise ValueError(f"No changelog entry for {version}")


def validate(tag, sha):
    if not re.fullmatch(r"v\d+\.\d+\.\d+", tag):
        raise ValueError("Expected a stable vX.Y.Z tag")
    if not re.fullmatch(r"[a-f0-9]{40}", sha):
        raise ValueError("Expected a full commit SHA")
    if run("git", "rev-parse", "HEAD") != sha:
        raise ValueError("Checkout does not match the release commit")
    if run("gh", "api", f"repos/{os.environ['GH_REPO']}/commits/{tag}", "--jq", ".sha") != sha:
        raise ValueError("Release tag moved")
    version = tomllib.loads(Path("Cargo.toml").read_text())["workspace"]["package"]["version"]
    if tag != f"v{version}":
        raise ValueError("Tag does not match workspace version")
    return notes(version, Path("CHANGELOG.md").read_text())


def release_state(tag):
    result = subprocess.run(
        ["gh", "api", f"repos/{os.environ['GH_REPO']}/releases/tags/{tag}"],
        text=True, capture_output=True,
    )
    if result.returncode:
        if "(HTTP 404)" in result.stderr:
            return None
        raise RuntimeError(result.stderr)
    return json.loads(result.stdout)


def prepare(tag, sha):
    body = validate(tag, sha)
    state = release_state(tag)
    if state is None:
        Path("notes.md").write_text(body)
        run("gh", "release", "create", tag, "--draft", "--verify-tag", "--target", sha,
            "--title", f"Romlin {tag[1:]}", "--notes-file", "notes.md")
        return True
    return state["draft"]


def publish(tag, sha):
    body = validate(tag, sha)
    state = release_state(tag)
    if state is None:
        raise ValueError("Draft release is missing; run preparation again")
    if not state["draft"]:
        return  # A retry must never replace an already published release's assets.
    assets = sorted(str(path) for path in Path("dist").iterdir() if path.is_file())
    if not assets:
        raise ValueError("No release assets")
    Path("notes.md").write_text(body)
    run("gh", "release", "upload", tag, *assets, "--clobber")
    validate(tag, sha)
    run("gh", "release", "edit", tag, "--title", f"Romlin {tag[1:]}",
        "--notes-file", "notes.md", "--draft=false", "--latest")


if __name__ == "__main__":
    command, *args = sys.argv[1:]
    if command == "notes":
        print(notes(args[0].removeprefix("v"), Path("CHANGELOG.md").read_text()), end="")
    elif command == "prepare":
        active = prepare(*args)
        with open(os.environ["GITHUB_OUTPUT"], "a") as output:
            output.write(f"active={str(active).lower()}\n")
    elif command == "publish":
        publish(*args)
    else:
        raise ValueError(f"Unknown release command: {command}")
