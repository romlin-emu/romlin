"""Offline checks: python3 -m unittest discover -s packaging/steamos."""
import hashlib
import io
import json
from pathlib import Path
import ctypes
import ctypes.util
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import install


def desktop_args(command):
    # GLib applies desktop Exec quoting after the desktop string unescape pass.
    glib = ctypes.CDLL(ctypes.util.find_library('glib-2.0'))
    argc = ctypes.c_int()
    argv = ctypes.POINTER(ctypes.c_char_p)()
    success = glib.g_shell_parse_argv(command.replace('\\\\', '\\').encode(),
                                      ctypes.byref(argc), ctypes.byref(argv), None)
    if not success:
        raise ValueError('Invalid desktop command')
    try:
        return [argv[i].decode() for i in range(argc.value)]
    finally:
        glib.g_strfreev(argv)


class InstallerTests(unittest.TestCase):
    def run_install(self, home, corrupt=False, add=True, existing=False):
        name = 'Romlin-nightly-x86_64.AppImage'
        url = 'https://github.com/romlin-emu/romlin/releases/download/nightly/'
        release = {'assets': [{'name': n, 'browser_download_url': url + n}
                              for n in (name, name + '.sha512')]}
        payload = b'appimage fixture'

        def download(source, dest):
            if source.endswith('.sha512'):
                digest = 'bad' if corrupt else hashlib.sha512(payload).hexdigest()
                dest.write_text(digest + '  ' + name)
            else:
                dest.write_bytes(payload)

        def run(args, **kwargs):
            return subprocess.CompletedProcess(args, 0 if add or '--yesno' not in args else 1)

        with patch.object(install.platform, 'system', return_value='Linux'), \
             patch.object(install.platform, 'machine', return_value='x86_64'), \
             patch.object(install.os, 'geteuid', return_value=1000), \
             patch.object(install.shutil, 'which', return_value='/usr/bin/steamos-add-to-steam'), \
             patch.object(install.Path, 'home', return_value=home), \
             patch.dict(install.os.environ, {'XDG_DATA_HOME': str(home / 'data')}), \
             patch.object(install.urllib.request, 'urlopen', return_value=io.BytesIO(json.dumps(release).encode())) as fetch, \
             patch.object(install, 'download', side_effect=download), \
             patch.object(install.subprocess, 'run', side_effect=run) as calls, \
             patch.object(install, 'install_artwork') as artwork, \
             patch.object(install, 'steam_shortcuts', return_value=[('grid', 321)] if existing else []):

            install.install()
            if add or existing:
                artwork.assert_called_once_with(home / "Applications/Romlin")
            else:
                artwork.assert_not_called()
            fetch.assert_called_once()
            self.assertEqual(fetch.call_args.args[0].full_url,
                             'https://api.github.com/repos/romlin-emu/romlin/releases/tags/nightly')
            return calls

    def test_install_and_update_without_duplicate(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp) / 'home with spaces'
            calls = self.run_install(home)
            image = home / 'Applications/Romlin/Romlin.AppImage'
            self.assertEqual(image.read_bytes(), b'appimage fixture')
            self.assertTrue(image.stat().st_mode & 0o111)
            launcher = home / 'data/applications/io.github.romlinemu.romlin.desktop'
            self.assertIn('--appimage-extract-and-run', launcher.read_text())
            helper_calls = [c.args[0] for c in calls.call_args_list
                            if c.args[0][0] == '/usr/bin/steamos-add-to-steam']
            self.assertEqual(helper_calls, [['/usr/bin/steamos-add-to-steam', str(launcher)]])
            calls = self.run_install(home, existing=True)
            self.assertTrue(all(c.args[0][0] == 'kdialog' for c in calls.call_args_list))
            self.assertFalse(any('--yesno' in c.args[0] for c in calls.call_args_list))

    def test_declining_steam_add_skips_artwork(self):
        with tempfile.TemporaryDirectory() as temp:
            calls = self.run_install(Path(temp), add=False)
            self.assertTrue(all(c.args[0][0] == 'kdialog' for c in calls.call_args_list))

    def test_checksum_failure_preserves_old_app(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            image = home / 'Applications/Romlin/Romlin.AppImage'
            image.parent.mkdir(parents=True)
            image.write_bytes(b'old app')
            with self.assertRaisesRegex(RuntimeError, 'checksum'):
                self.run_install(home, corrupt=True)
            self.assertEqual(image.read_bytes(), b'old app')
            self.assertEqual(list(image.parent.iterdir()), [image])

    def test_unexpected_download_source_rejected(self):
        for url in [
            'https://example.com/app',
            'https://github.com/RompEmu/RomP/releases/download/nightly/RomP-nightly-x86_64.AppImage',
        ]:
            with self.subTest(url=url), self.assertRaises(ValueError):
                install.download(url, Path('/unused'))

    def test_desktop_quoting_and_bootstrap_syntax(self):
        value = '/home/a b/$`"\\/Romlin.AppImage'
        self.assertEqual(desktop_args(install.desktop_quote(value)), [value])
        desktop = Path(__file__).with_name('Install-Romlin.desktop').read_text()
        command = next(line[5:] for line in desktop.splitlines() if line.startswith('Exec='))
        args = desktop_args(command)
        self.assertEqual(args[:2], ['bash', '-c'])
        subprocess.run(['bash', '-n', '-c', args[2]], check=True)


class ArtworkTests(unittest.TestCase):
    def fixture(self, executable, appid=321):
        def text(key, value):
            return b'\x01' + key.encode() + b'\0' + value.encode() + b'\0'
        return (b'\x00shortcuts\0\x000\0\x02appid\0' + appid.to_bytes(4, 'little')
                + text('AppName', 'Romlin') + text('Exe', executable)
                + b'\x00tags\0\x08\x08\x08\x08')

    def test_artwork_matches_saved_shortcut_and_refreshes_art(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            folder = home / 'Applications/Romlin'
            folder.mkdir(parents=True)
            config = home / '.local/share/Steam/userdata/123/config'
            config.mkdir(parents=True)
            database = self.fixture('"' + str(folder / 'Romlin.AppImage') + '"')
            (config / 'shortcuts.vdf').write_bytes(database)
            grid = config / 'grid'
            grid.mkdir()
            (grid / '321p.jpg').write_bytes(b'custom')
            (grid / '321_hero.png').write_bytes(b'old hero')
            payload = b'\x89PNG\r\n\x1a\nfixture'
            with patch.object(install.Path, 'home', return_value=home), \
                 patch.object(install.subprocess, 'run'), \
                 patch.object(install.urllib.request, 'urlopen', side_effect=lambda *a, **k: io.BytesIO(payload)):
                install.install_artwork(folder)
                install.install_artwork(folder)
            self.assertEqual((grid / '321_hero.png').read_bytes(), payload)
            self.assertEqual((grid / '321_logo.png').read_bytes(), payload)
            self.assertEqual((grid / '321.png').read_bytes(), payload)
            self.assertEqual((grid / '321p.png').read_bytes(), payload)
            self.assertFalse((grid / '321p.jpg').exists())
            self.assertEqual((config / 'shortcuts.vdf').read_bytes(), database)

    def test_malformed_shortcuts_rejected(self):
        for data in (b'', b'\x00shortcuts', b'\x03bad\0', b'\x02appid\0\x01'):
            with self.subTest(data=data), self.assertRaises((ValueError, IndexError)):
                install.read_shortcuts(data)

    def test_missing_shortcut_is_actionable(self):
        with tempfile.TemporaryDirectory() as temp, \
             patch.object(install.Path, 'home', return_value=Path(temp)), \
             patch.object(install.subprocess, 'run'):
            with self.assertRaisesRegex(RuntimeError, 'saved Steam shortcut was not found'):
                install.install_artwork(Path(temp))
