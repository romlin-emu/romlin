#!/usr/bin/env python3
"""Install the nightly Romlin AppImage and Steam library artwork without modifying SteamOS."""

import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import tempfile
import urllib.request


REPO = "romlin-emu/romlin"


def download(url, target):
    if not url.startswith(f"https://github.com/{REPO}/releases/download/"):
        raise ValueError("Unexpected release download URL")
    with urllib.request.urlopen(url, timeout=60) as source, target.open("wb") as dest:
        shutil.copyfileobj(source, dest)


def desktop_quote(value):
    # Desktop Exec has its own quoting rules, independent of shell quoting.
    if any(c in value for c in "\n\r\0"):
        raise ValueError("Unsupported character in installation path")
    quoted = ''.join('\\' + c if c in '\\"`$' else c for c in value)
    return '"' + quoted.replace('\\', '\\\\') + '"'


def read_shortcuts(data):
    """Read Steam's binary KeyValues; never rewrite its shortcut database."""
    offset = 0

    def string():
        nonlocal offset
        end = data.index(b"\0", offset)
        value = data[offset:end].decode("utf-8")
        offset = end + 1
        return value

    def object_(depth=0):
        nonlocal offset
        if depth > 16:
            raise ValueError("Steam shortcut nesting is too deep")
        result = {}
        while True:
            kind = data[offset]
            offset += 1
            if kind == 8:
                return result
            key = string().lower()
            if kind == 0:
                result[key] = object_(depth + 1)
            elif kind == 1:
                result[key] = string()
            elif kind == 2:
                if offset + 4 > len(data):
                    raise ValueError("Truncated Steam shortcut integer")
                result[key] = int.from_bytes(data[offset:offset + 4], "little")
                offset += 4
            else:
                raise ValueError(f"Unsupported Steam shortcut value type: {kind}")

    return object_().get("shortcuts", {})


def steam_shortcuts(folder):
    roots = {p.resolve() for p in (
        Path.home() / ".steam/steam", Path.home() / ".steam/root",
        Path.home() / ".local/share/Steam",
    ) if p.is_dir()}
    matches = []
    for root in roots:
        for database in (root / "userdata").glob("*/config/shortcuts.vdf"):
            for entry in read_shortcuts(database.read_bytes()).values():
                if (isinstance(entry, dict) and entry.get("appname") == "Romlin"
                        and str(folder / "Romlin.AppImage") in entry.get("exe", "")
                        and isinstance(entry.get("appid"), int)):
                    matches.append((database.parent / "grid", entry["appid"]))
    return matches


def install_artwork(folder):
    matches = steam_shortcuts(folder)
    if not matches:
        raise RuntimeError("Romlin is installed, but its saved Steam shortcut was not found. "
                           "Exit Steam and rerun the installer.")
    artwork = {"capsule.png": "p", "header.png": "", "hero.png": "_hero",
               "logo.png": "_logo", "icon.png": "_icon"}
    with tempfile.TemporaryDirectory(prefix=".artwork-", dir=folder) as staging:
        for name in artwork:
            target = Path(staging) / name
            url = f"https://raw.githubusercontent.com/{REPO}/main/packaging/steamos/artwork/{name}"
            with urllib.request.urlopen(url, timeout=60) as source, target.open("wb") as dest:
                shutil.copyfileobj(source, dest)
            if not target.read_bytes().startswith(b"\x89PNG\r\n\x1a\n"):
                raise ValueError(f"Invalid artwork download: {name}")
        for grid, appid in matches:
            grid.mkdir(parents=True, exist_ok=True)
            for name, suffix in artwork.items():
                target = grid / f"{appid}{suffix}.png"
                # Stage beside the destination so replacement is atomic.
                with tempfile.NamedTemporaryFile(dir=grid, delete=False) as dest:
                    staged = Path(dest.name)
                    try:
                        dest.write((Path(staging) / name).read_bytes())
                        dest.flush()
                        staged.replace(target)
                    finally:
                        staged.unlink(missing_ok=True)
                for extension in ("jpg", "jpeg", "webp", "tga"):
                    target.with_suffix("." + extension).unlink(missing_ok=True)
    subprocess.run(["kdialog", "--title", "Romlin ready", "--msgbox",
                    "Steam artwork installed. Restart Steam or return to Gaming Mode."], check=True)


def install():
    if platform.system() != "Linux" or platform.machine() != "x86_64":
        raise RuntimeError("This installer needs x86_64 Linux (SteamOS).")
    if os.geteuid() == 0:
        raise RuntimeError("Run this installer as your normal user, without sudo.")
    helper = shutil.which("steamos-add-to-steam")
    if not helper:
        raise RuntimeError("SteamOS's steamos-add-to-steam helper was not found.")

    subprocess.run(["kdialog", "--title", "Install Romlin", "--msgbox",
                    "Romlin will download the latest nightly build to ~/Applications/Romlin. "
                    "An existing installation there will be updated. Close Romlin before continuing. "
                    "No password is needed. The download may take a few minutes."], check=True)
    request = urllib.request.Request(
        f"https://api.github.com/repos/{REPO}/releases/tags/nightly",
        headers={"Accept": "application/vnd.github+json", "User-Agent": "Romlin-installer"},
    )
    with urllib.request.urlopen(request, timeout=60) as response:
        release = json.load(response)
    assets = {a["name"]: a["browser_download_url"] for a in release["assets"]}
    images = [name for name in assets if name.endswith("-x86_64.AppImage")]
    if len(images) != 1 or images[0] + ".sha512" not in assets:
        raise RuntimeError("The nightly release has no x86_64 AppImage and checksum.")
    name = images[0]
    folder = Path.home() / "Applications" / "Romlin"
    folder.mkdir(parents=True, exist_ok=True)
    # Stage on the same filesystem, so a failed download leaves the old app intact.
    with tempfile.TemporaryDirectory(prefix=".install-", dir=folder) as staging:
        image = Path(staging) / name
        checksum = Path(staging) / "checksum"
        download(assets[name], image)
        download(assets[name + ".sha512"], checksum)
        expected = checksum.read_text().split()[0]
        with image.open("rb") as source:
            digest = hashlib.sha512()
            for block in iter(lambda: source.read(1024 * 1024), b""):
                digest.update(block)
        if digest.hexdigest() != expected:
            raise RuntimeError("Download checksum failed; your existing app was kept.")
        image.chmod(0o755)
        image.replace(folder / "Romlin.AppImage")

    applications = Path(os.environ.get("XDG_DATA_HOME", Path.home() / ".local/share")) / "applications"
    applications.mkdir(parents=True, exist_ok=True)
    launcher = applications / "io.github.romlinemu.romlin.desktop"
    # Percent is a desktop field-code marker even inside quotes.
    executable = desktop_quote(str(folder / "Romlin.AppImage").replace("%", "%%"))
    launcher.write_text(
        "[Desktop Entry]\nType=Application\nName=Romlin\n"
        "Comment=Play retro games from your RomM library\n"
        f"Exec={executable} --appimage-extract-and-run\n"
        "Icon=applications-games\nCategories=Game;Emulator;\nTerminal=false\n"
    )
    launcher.chmod(0o755)
    subprocess.run(["kdialog", "--title", "Romlin Steam artwork", "--msgbox",
                    "Exit Steam using Steam → Exit, then click OK. This saves any existing "
                    "Romlin shortcut so the installer can reuse it and update its artwork."], check=True)
    if not steam_shortcuts(folder):
        add = subprocess.run(["kdialog", "--title", "Romlin installed", "--yesno",
                              "Add Romlin to Steam now?"])
        if add.returncode == 0:
            subprocess.run([helper, str(launcher)], check=True)
            subprocess.run(["kdialog", "--title", "Romlin Steam artwork", "--msgbox",
                            "Finish adding Romlin, then exit Steam using Steam → Exit. "
                            "Click OK once Steam has closed to install the artwork."], check=True)
        elif add.returncode == 1:
            return
        else:
            raise RuntimeError("Could not display the Add to Steam prompt.")
    install_artwork(folder)


if __name__ == "__main__":
    try:
        install()
    except Exception as error:
        message = f"Romlin installation could not finish: {error}\nYou can run the installer again."
        print(message, flush=True)
        subprocess.run(["kdialog", "--title", "Install Romlin", "--error", message])
        raise SystemExit(1)
