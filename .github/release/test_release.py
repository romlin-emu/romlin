import os
import hashlib
import re
import sys
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import release


class ReleaseTests(unittest.TestCase):
    def test_notes_support_both_formats_and_stop_at_next_release(self):
        for header in ('## [0.20.0] - 2026-11-02', '## [0.20.0](url) (2026-11-02)', '### [0.20.0](url) (2026-11-02)', '## 0.20.0 (2026-11-02)'):
            text = f'# Changelog\n{header}\n\n### Features\n\n- A change\n\n## [0.19.0]\nold\n'
            self.assertEqual(release.notes('0.20.0', text), '### Features\n\n- A change\n')
        with self.assertRaises(ValueError):
            release.notes('9.0.0', text)

    def test_smoke_checks_actual_fake08_download_before_extraction(self):
        workflow = Path('.github/workflows/smoke.yml').read_text()
        script = re.search(r'"\$python" -c \'(import hashlib, pathlib;.*?)\'', workflow)[1]
        previous = Path.cwd()
        with tempfile.TemporaryDirectory() as directory:
            os.chdir(directory)
            try:
                Path('core.zip').write_bytes(b'archive')
                checksum = hashlib.sha256(b'archive').hexdigest()
                Path('core.zip.sha256').write_text(checksum + '  fake08_libretro-linux-x86_64.zip\n')
                subprocess.run([sys.executable, '-c', script], check=True)
                Path('core.zip').write_bytes(b'corrupt')
                result = subprocess.run([sys.executable, '-c', script], capture_output=True)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn(b'FAKE-08 checksum mismatch', result.stderr)
            finally:
                os.chdir(previous)

    @patch.dict(os.environ, GH_REPO='romlin-emu/romlin')
    @patch('release.run')
    def test_wrong_checkout_and_moved_tag_fail_before_publication(self, run):
        run.return_value = 'b' * 40
        with self.assertRaisesRegex(ValueError, 'Checkout'):
            release.validate('v0.19.0', 'a' * 40)
        run.side_effect = ['a' * 40, 'b' * 40]
        with self.assertRaisesRegex(ValueError, 'tag moved'):
            release.validate('v0.19.0', 'a' * 40)
        run.reset_mock()
        with self.assertRaises(ValueError):
            release.validate('main', 'a' * 40)
        run.assert_not_called()

    @patch('release.validate', return_value='notes\n')
    @patch('release.release_state')
    @patch('release.run')
    def test_failed_upload_stays_draft_and_retry_uploads_before_publish(self, run, state, validate):
        state.return_value = {'draft': True}
        previous = Path.cwd()
        with tempfile.TemporaryDirectory() as directory:
            os.chdir(directory)
            try:
                Path('dist').mkdir()
                Path('dist/app.zip').write_text('archive')
                run.side_effect = subprocess.CalledProcessError(1, 'upload')
                with self.assertRaises(subprocess.CalledProcessError):
                    release.publish('v0.20.0', 'a' * 40)
                self.assertEqual(run.call_args.args[2], 'upload')
                self.assertEqual(run.call_count, 1)
                run.reset_mock()
                run.side_effect = None
                release.publish('v0.20.0', 'a' * 40)
                self.assertEqual([call.args[2] for call in run.call_args_list], ['upload', 'edit'])
                self.assertIn('--clobber', run.call_args_list[0].args)
                self.assertIn('--draft=false', run.call_args.args)
                self.assertIn('--latest', run.call_args.args)
                state.return_value = {'draft': False}
                run.reset_mock()
                release.publish('v0.20.0', 'a' * 40)
                run.assert_not_called()
            finally:
                os.chdir(previous)

    @patch('release.validate', return_value='notes\n')
    @patch('release.release_state')
    @patch('release.run')
    def test_prepare_reuses_draft_and_does_not_recreate_published_release(self, run, state, validate):
        for draft in (True, False):
            state.return_value = {'draft': draft}
            self.assertEqual(release.prepare('v0.20.0', 'a' * 40), draft)
        run.assert_not_called()

    @patch.dict(os.environ, GH_REPO='romlin-emu/romlin')
    @patch('release.subprocess.run')
    def test_only_not_found_allows_draft_creation(self, run):
        run.return_value = subprocess.CompletedProcess([], 1, '', 'network error')
        with self.assertRaises(RuntimeError):
            release.release_state('v0.20.0')
        run.return_value = subprocess.CompletedProcess([], 1, '', 'gh: Not Found (HTTP 404)')
        self.assertIsNone(release.release_state('v0.20.0'))


if __name__ == '__main__':
    unittest.main()
