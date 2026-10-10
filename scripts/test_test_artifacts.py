import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

from test_artifacts import preserve


class FailureArtifactTests(unittest.TestCase):
    def test_real_child_exit_and_stderr_are_preserved_before_fixture_cleanup(self):
        with tempfile.TemporaryDirectory() as outer:
            base = Path(outer)
            fixture = base / 'fixture'
            fixture.mkdir()
            log = fixture / 'child.stderr'
            with log.open('wb') as output:
                child = subprocess.Popen([sys.executable, '-c', "import sys; print('startup error canary', file=sys.stderr); sys.exit(23)"], stdout=output, stderr=output)
                child.wait(timeout=5)
            context = {'status.last_error': {'type': 'ConnectionRefusedError', 'errno': 111}}
            artifact = preserve(fixture, [child], context, [Path(sys.executable)], 'legacy startup deadline', base / 'artifacts')
            report = json.loads((artifact / 'report.json').read_text())
            self.assertEqual(report['processes'][0]['exit_code'], 23)
            self.assertEqual(report['context'], context)
            self.assertEqual(report['binaries'][0]['bytes'], Path(sys.executable).stat().st_size)
            self.assertEqual(len(report['binaries'][0]['sha256']), 64)
            self.assertIn('startup error canary', (artifact / 'files/child.stderr').read_text())

    def test_copy_is_bounded_skips_symlinks_binaries_and_non_regular_files(self):
        with tempfile.TemporaryDirectory() as outer:
            base = Path(outer)
            fixture = base / 'fixture'
            fixture.mkdir()
            (fixture / 'large.log').write_bytes(b'x' * (600 * 1024) + b'tail canary')
            (fixture / 'application.exe').write_bytes(b'not a diagnostic file')
            (base / 'outside.json').write_text('must not follow a symlink')
            if os.name == 'posix':
                (fixture / 'link.json').symlink_to(base / 'outside.json')
                os.mkfifo(fixture / 'blocked.log')
            artifact = preserve(fixture, [], {'phase': 'rollback'}, [], 'injected deadline', base / 'artifacts')
            report = json.loads((artifact / 'report.json').read_text())
            self.assertEqual((artifact / 'files/large.log').stat().st_size, 512 * 1024)
            self.assertTrue((artifact / 'files/large.log').read_bytes().endswith(b'tail canary'))
            self.assertTrue(any('truncated tail' in omission for omission in report['omissions']))
            self.assertFalse((artifact / 'files/application.exe').exists())
            self.assertFalse((artifact / 'files/link.json').exists())
            self.assertFalse((artifact / 'files/blocked.log').exists())

    def test_artifact_storage_error_does_not_raise_a_second_failure(self):
        with tempfile.TemporaryDirectory() as outer:
            base = Path(outer)
            blocked = base / 'file-not-directory'
            blocked.write_text('unwritable destination')
            self.assertIsNone(preserve(base, [], {}, [], 'original failure', blocked))
