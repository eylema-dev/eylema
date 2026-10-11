"""Failure tests for development doctor; mocked commands are not hardware evidence."""
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from doctor.doctor import (DoctorError, EXPECTED_VERSIONS, HOST, RUST_COMMIT, TOOLCHAIN,
                           Runner, artifact, collect, probe_workspace, public_text,
                           rust_identity, validate_versions)


class DoctorTests(unittest.TestCase):
    def test_every_required_tool_rejects_empty_wrong_and_missing_identity(self):
        validate_versions(dict(EXPECTED_VERSIONS))
        for name in EXPECTED_VERSIONS:
            for value in ['', 'unexpected tool', EXPECTED_VERSIONS[name] + ' modified']:
                with self.subTest(tool=name, value=value), self.assertRaises(DoctorError):
                    validate_versions(dict(EXPECTED_VERSIONS, **{name: value}))
            missing = dict(EXPECTED_VERSIONS)
            del missing[name]
            with self.assertRaises(DoctorError): validate_versions(missing)

    def test_probe_filesystem_error_remains_redacted_in_failed_cli_json(self):
        from doctor.doctor import main
        observed = {}
        def fail():
            with probe_workspace() as scratch:
                observed['scratch'] = str(scratch)
                raise PermissionError(13, 'Permission denied', str(scratch / 'native.rs'))
        with patch('sys.argv', ['doctor']), patch('doctor.doctor.collect', side_effect=fail), patch('builtins.print') as output:
            self.assertEqual(main(), 1)
        result = json.loads(output.call_args.args[0])
        self.assertEqual(result['status'], 'failed')
        self.assertNotIn(observed['scratch'], result['reason'])
        self.assertIn('$PROBE/native.rs', result['reason'])

    def test_rust_pin_and_native_host_reject_drift_and_duplicate_fields(self):
        valid = f'release: {TOOLCHAIN}\nhost: {HOST}\ncommit-hash: ' + RUST_COMMIT
        self.assertEqual(rust_identity(valid)['release'], TOOLCHAIN)
        for output in [valid.replace(TOOLCHAIN, '1.98.0'), valid.replace(HOST, 'x86_64-apple-darwin'),
                       valid.replace(RUST_COMMIT, 'a' * 40), valid.replace(RUST_COMMIT, 'short'), valid + '\nrelease: 1.99.0', '']:
            with self.subTest(output=output), self.assertRaises(DoctorError):
                rust_identity(output)

    def test_failed_missing_and_timed_out_commands_never_record_pass(self):
        with tempfile.TemporaryDirectory() as temporary:
            runner = Runner(Path(temporary))
            with patch('doctor.doctor.subprocess.run', return_value=subprocess.CompletedProcess(['tool'], 7, 'invented success', 'failed')):
                with self.assertRaises(DoctorError): runner.run(['tool'])
            for failure in [FileNotFoundError(), subprocess.TimeoutExpired(['tool'], 120)]:
                with patch('doctor.doctor.subprocess.run', side_effect=failure):
                    with self.assertRaises(DoctorError): runner.run(['tool'])
            self.assertEqual(runner.records, [])

    def test_missing_empty_and_invalid_artifact_fail(self):
        with tempfile.TemporaryDirectory() as temporary:
            p = Path(temporary) / 'probe.wasm'
            with self.assertRaises(DoctorError): artifact(p)
            for contents in [b'', b'not wasm', b'\x00asm\x02\x00\x00\x00']:
                p.write_bytes(contents)
                with self.assertRaises(DoctorError): artifact(p, b'\x00asm\x01\x00\x00\x00')
            p.write_bytes(b'\x00asm\x01\x00\x00\x00')
            self.assertEqual(artifact(p, b'\x00asm\x01\x00\x00\x00')['bytes'], 8)

    def test_unsupported_host_fails_before_commands_and_redacts_private_paths(self):
        with patch('doctor.doctor.platform.system', return_value='Linux'), patch('doctor.doctor.subprocess.run') as command:
            with self.assertRaises(DoctorError): collect()
            command.assert_not_called()
        home = str(Path.home())
        self.assertEqual(public_text(home + '/file /scratch/probe', Path('/scratch')), '$HOME/file $PROBE/probe')
        self.assertEqual(public_text('/tmp/eylema-doctor-test/native.rs', Path('/private/tmp/eylema-doctor-test')), '$PROBE/native.rs')
        self.assertEqual(public_text('InstalledDir: /private/transient/mount'), 'InstalledDir: $METAL_TOOLCHAIN')
        self.assertEqual(public_text('/var/run/mnt/com.apple.MobileAsset.MetalToolchain-version/usr/bin/metallib'), '$METAL_TOOLCHAIN/metallib')

    def test_failed_cli_preserves_success_record_and_returns_nonzero(self):
        from doctor.doctor import main
        with tempfile.TemporaryDirectory() as temporary:
            p = Path(temporary) / 'report.json'
            p.write_text('prior evidence')
            with patch('sys.argv', ['doctor', '--output', str(p)]), patch('doctor.doctor.collect', side_effect=DoctorError('missing required tool')), patch('builtins.print') as output:
                self.assertEqual(main(), 1)
            self.assertEqual(json.loads(output.call_args.args[0])['status'], 'failed')
            self.assertEqual(p.read_text(), 'prior evidence')

    def test_storage_failure_preserves_prior_report_and_cleans_temporary_file(self):
        from doctor.doctor import main
        with tempfile.TemporaryDirectory() as temporary:
            p = Path(temporary) / 'report.json'
            p.write_text('prior evidence')
            with patch('sys.argv', ['doctor', '--output', str(p)]), patch('doctor.doctor.collect', return_value={'status': 'passed'}), patch('doctor.doctor.os.replace', side_effect=OSError('write failed')), patch('builtins.print') as output:
                self.assertEqual(main(), 1)
            self.assertEqual(json.loads(output.call_args.args[0])['status'], 'failed')
            self.assertEqual(p.read_text(), 'prior evidence')
            self.assertEqual(list(Path(temporary).iterdir()), [p])
