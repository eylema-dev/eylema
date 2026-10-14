"""Development-only environment probes; not an Eyleme runtime or GPU test."""
import argparse
import datetime
import hashlib
import json
import os
import platform
import plistlib
import re
import subprocess
import sys
import tempfile
from contextlib import contextmanager
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from check_rust import validate_environment

TOOLCHAIN = '1.99.0'
HOST = 'aarch64-apple-darwin'
RUST_COMMIT = 'b940084d7eb6a299eb4bfeb8e34901bc051e7ac4'
WASM = 'wasm32-unknown-unknown'
EXPECTED_VERSIONS = {
    'rustup': 'rustup 1.29.1 (d95a37b6a 2026-08-13)',
    'cargo': 'cargo 1.99.0 (5f94df478 2026-08-27)',
    'rustfmt': 'rustfmt 1.10.0-stable (b940084d7e 2026-09-28)',
    'clippy': 'clippy 0.1.99 (b940084d7e 2026-09-28)',
}
NATIVE_SOURCE = 'fn main() { println!("G04-native-arm64-ok"); }\n'
WASM_SOURCE = '#[unsafe(no_mangle)]\npub extern "C" fn eylema_doctor_add(a: u32, b: u32) -> u32 { a.wrapping_add(b) }\n'
METAL_SOURCE = '''#include <metal_stdlib>
using namespace metal;
kernel void doctor_add(device uint *output [[buffer(0)]], uint index [[thread_position_in_grid]]) {
    if (index < 4) output[index] = index + 1;
}
'''


class DoctorError(ValueError):
    pass


def require(condition, message):
    if not condition:
        raise DoctorError(message)


def sha256(value):
    return hashlib.sha256(value).hexdigest()


def public_text(value, scratch=None):
    value = value.replace(str(Path.home()), '$HOME')
    if scratch:
        value = value.replace(str(scratch), '$PROBE')
        if str(scratch).startswith('/private/'):
            value = value.replace(str(scratch).removeprefix('/private'), '$PROBE')
    # Cryptex mount suffixes are transient paths, not stable tool identities.
    value = re.sub(r'(?m)^InstalledDir: .+$', 'InstalledDir: $METAL_TOOLCHAIN', value)
    if value.startswith('/') and '.MetalToolchain-' in value:
        value = '$METAL_TOOLCHAIN/' + Path(value).name
    return value


class Runner:
    def __init__(self, scratch):
        self.scratch = scratch
        self.records = []

    def run(self, command):
        shown = [public_text(str(part), self.scratch) for part in command]
        try:
            result = subprocess.run(command, cwd=self.scratch, capture_output=True,
                                    text=True, timeout=120, check=False)
        except (OSError, subprocess.TimeoutExpired) as exc:
            raise DoctorError(f'Unavailable or timed-out required command: {shown[0]}') from exc
        require(result.returncode == 0, 'Required command failed: ' + ' '.join(shown)
                + '\n' + public_text(result.stderr[:2000], self.scratch))
        self.records.append({'command': shown, 'status': 'passed',
                             'stdout': public_text(result.stdout.strip(), self.scratch)})
        return result.stdout.strip()


def rust_identity(output):
    values = {}
    for line in output.splitlines():
        key, sep, value = line.partition(': ')
        if sep:
            require(key not in values, 'Duplicate rustc version field')
            values[key] = value
    require(values.get('release') == TOOLCHAIN, 'Expected exact Rust 1.99.0 release')
    require(values.get('host') == HOST, 'Expected native aarch64-apple-darwin compiler')
    require(values.get('commit-hash') == RUST_COMMIT, 'Compiler commit differs from the pinned official release')
    return values


def validate_versions(versions):
    require(set(versions) == set(EXPECTED_VERSIONS), 'Required tool identity inventory is incomplete')
    for name, expected in EXPECTED_VERSIONS.items():
        require(versions[name] == expected, 'Required tool identity differs from pin: ' + name)


@contextmanager
def probe_workspace():
    scratch = None
    try:
        with tempfile.TemporaryDirectory(prefix='eylema-doctor-') as temporary:
            scratch = Path(temporary).resolve()
            yield scratch
    except OSError as exc:
        # Preserve the workspace boundary through probe access and cleanup.
        raise DoctorError(public_text(str(exc), scratch)) from exc


def artifact(path, magic=None):
    require(path.is_file(), 'Required probe output was not created')
    value = path.read_bytes()
    require(bool(value), 'Required probe output is empty')
    if magic:
        require(value.startswith(magic), 'Required probe output has wrong format')
    return {'bytes': len(value), 'sha256': sha256(value)}


def collect():
    require(platform.system() == 'Darwin' and platform.machine() in {'arm64', 'aarch64'},
            'This doctor requires native ARM64 macOS; no cross-host acceptance')
    validate_environment()
    rustup = Path(os.environ.get('CARGO_HOME', str(Path.home() / '.cargo'))) / 'bin/rustup'
    require(rustup.is_file(), 'Install official rustup before running doctor')
    with probe_workspace() as scratch:
        runner = Runner(scratch)
        rust = [str(rustup), 'run', TOOLCHAIN]
        identity = rust_identity(runner.run(rust + ['rustc', '--version', '--verbose']))
        versions = {
            'rustup': runner.run([str(rustup), '--version']),
            'cargo': runner.run(rust + ['cargo', '--version']),
            'rustfmt': runner.run(rust + ['rustfmt', '--version']),
            'clippy': runner.run(rust + ['cargo-clippy', '--version']),
        }
        validate_versions(versions)
        installed = runner.run([str(rustup), 'target', 'list', '--toolchain', TOOLCHAIN, '--installed']).splitlines()
        require({HOST, WASM} <= set(installed), 'Required native/Wasm target missing')
        chip = runner.run(['/usr/sbin/sysctl', '-n', 'machdep.cpu.brand_string'])
        memory = runner.run(['/usr/sbin/sysctl', '-n', 'hw.memsize'])
        require(chip == 'Apple M4 Pro' and memory == '25769803776', 'Expected one M4 Pro/24 GB acceptance device')
        os_version = runner.run(['/usr/bin/sw_vers', '-productVersion'])
        xcode = runner.run(['/usr/bin/xcodebuild', '-version'])
        sdk = runner.run(['/usr/bin/xcrun', '--sdk', 'macosx', '--show-sdk-version'])
        require(bool(os_version and xcode.startswith('Xcode ') and sdk), 'Missing OS/Xcode/SDK inventory')
        metal = runner.run(['/usr/bin/xcrun', 'metal', '--version'])
        require(metal.startswith('Apple metal version '), 'Missing actual Metal compiler invocation')
        runner.run(['/usr/bin/xcrun', '--find', 'metallib'])
        chrome = Path('/Applications/Google Chrome.app/Contents/Info.plist')
        require(chrome.is_file(), 'Required Chrome inventory unavailable')
        browser = plistlib.loads(chrome.read_bytes())
        require(browser.get('CFBundleIdentifier') == 'com.google.Chrome'
                and bool(browser.get('CFBundleShortVersionString')), 'Invalid Chrome bundle inventory')

        native_source = scratch / 'native.rs'
        native_source.write_text(NATIVE_SOURCE)
        native = scratch / 'native'
        runner.run(rust + ['rustc', '--edition=2024', '--target', HOST, str(native_source), '-o', str(native)])
        file_type = runner.run(['/usr/bin/file', '-b', str(native)])
        require('Mach-O 64-bit executable arm64' in file_type, 'Probe binary is not native ARM64 Mach-O')
        require(runner.run([str(native)]) == 'G04-native-arm64-ok', 'Native executable returned wrong probe result')
        native_result = artifact(native)

        wasm_source = scratch / 'portable.rs'
        wasm_source.write_text(WASM_SOURCE)
        wasm = scratch / 'portable.wasm'
        runner.run(rust + ['rustc', '--edition=2024', '--crate-type=cdylib', '--crate-name', 'eylema_doctor_probe', '--target', WASM, str(wasm_source), '-o', str(wasm)])
        wasm_result = artifact(wasm, b'\x00asm\x01\x00\x00\x00')

        metal_source = scratch / 'probe.metal'
        metal_source.write_text(METAL_SOURCE)
        air = scratch / 'probe.air'
        library = scratch / 'probe.metallib'
        runner.run(['/usr/bin/xcrun', '--sdk', 'macosx', 'metal', '-std=metal4.0', '-c', str(metal_source), '-o', str(air)])
        air_result = artifact(air)
        runner.run(['/usr/bin/xcrun', '--sdk', 'macosx', 'metallib', str(air), '-o', str(library)])
        library_result = artifact(library, b'MTLB')

        return {
            'format_version': 1, 'goal': 'G04', 'status': 'passed',
            'observed_at': datetime.datetime.now(datetime.timezone.utc).isoformat(),
            'doctor_source_sha256': sha256(Path(__file__).read_bytes()),
            'hardware': {'physical_devices': 1, 'chip': chip, 'architecture': HOST,
                         'unified_memory_bytes': int(memory), 'os': 'macos', 'os_version': os_version},
            'toolchain': {'rust': identity, **versions, 'installed_targets': sorted(installed)},
            'platform': {'xcode': xcode, 'sdk_version': sdk,
                         'metal_version': metal.splitlines()[0],
                         'chrome': {'bundle_id': browser['CFBundleIdentifier'], 'version': browser['CFBundleShortVersionString']}},
            'probes': {
                'native_compile_run': {'status': 'passed', 'source_sha256': sha256(NATIVE_SOURCE.encode()), 'output': 'G04-native-arm64-ok', 'artifact': native_result},
                'wasm_compile': {'status': 'passed', 'source_sha256': sha256(WASM_SOURCE.encode()), 'artifact': wasm_result, 'execution': 'not_run'},
                'metal_compile_link': {'status': 'passed', 'source_sha256': sha256(METAL_SOURCE.encode()), 'air': air_result, 'library': library_result, 'gpu_execution': 'not_run'},
            },
            'commands': runner.records,
            'scope': 'Development smoke fixtures only; Eyleme runtime/workspace, browser/Wasm execution, GPU/model/training/performance acceptance not_run. Temporary probe artifacts are deleted.',
        }


def write_report(path, text):
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(mode='w', encoding='utf-8', dir=path.parent, delete=False) as file:
            temporary = Path(file.name)
            file.write(text)
            file.flush()
            os.fsync(file.fileno())
        os.replace(temporary, path)
    finally:
        if temporary:
            temporary.unlink(missing_ok=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, help='Write successful sanitized JSON inventory')
    args = parser.parse_args()
    try:
        report = collect()
        text = json.dumps(report, indent=2) + '\n'
        if args.output:
            write_report(args.output, text)
        else:
            print(text, end='')
    except (DoctorError, ValueError, OSError, plistlib.InvalidFileException) as exc:
        print(json.dumps({'goal': 'G04', 'status': 'failed', 'reason': public_text(str(exc))}))
        return 1
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
