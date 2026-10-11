#!/usr/bin/env node
// Development-only actual native/Wasm comparison; no browser acceptance claim.
import assert from 'node:assert/strict';
import { readFileSync, mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { execFileSync } from 'node:child_process';
import { webcrypto } from 'node:crypto';
import { codecHost, MAX_WIRE_BYTES } from '../platform/browser/codec-host.mjs';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
assert.equal(process.version, 'v26.11.1', 'Reviewed development Node version');
assert.equal(process.platform, 'darwin', 'Native CommonCrypto comparison requires macOS');
const run = (command, args, extra = {}) => execFileSync(command, args, { cwd: root, timeout: 600000, maxBuffer: 20 * 1024 * 1024, ...extra });
// Preflight stays in the unchanged first-party gate, including environment,
// trusted identity, exact toolchain and dependency/configuration validation.
run(join(root, '.venv/bin/python'), ['tools/check_rust.py'], { stdio: 'inherit' });
run('rustup', ['run', '1.99.0', 'cargo', 'build', '--package', 'eylema-platform', '--example', 'codec-proof', '--locked', '--offline'], { stdio: 'inherit' });
const native = join(root, 'target/debug/examples/codec-proof');
const wasmBytes = readFileSync(join(root, 'target/wasm32-unknown-unknown/debug/eylema_platform.wasm'));
const { instance } = await WebAssembly.instantiate(wasmBytes, {});
const host = codecHost(instance, webcrypto);
const hex = bytes => Buffer.from(bytes).toString('hex');
const temp = mkdtempSync(join(tmpdir(), 'eylema-g09-proof-'));
let accepted = 0, rejected = 0;
try {
  run(native, ['fixtures', temp]);
  const outputs = new Map();
  for (const line of readFileSync(join(temp, 'manifest.txt'), 'utf8').trim().split('\n')) {
    const [name, modeText, validText] = line.split('|'), mode = Number(modeText), valid = validText === 'true';
    const bytes = new Uint8Array(readFileSync(join(temp, `${name}.bin`)));
    const observed = JSON.parse(run(native, ['probe', modeText, join(temp, `${name}.bin`)]));
    assert.equal(observed.accepted, valid, `native ${name}`);
    if (valid) {
      const output = host.process(bytes, mode);
      assert.equal(hex(output), observed.bytes, `actual Wasm bytes ${name}`);
      assert.equal(hex(await host.digest(output)), observed.sha256, `actual WebCrypto/native SHA ${name}`);
      outputs.set(name, observed); accepted++;
    } else { assert.throws(() => host.process(bytes, mode), undefined, `Wasm rejection ${name}`); rejected++; }
  }
  assert.equal(accepted, 33); assert.equal(rejected, 30);
  assert.equal(outputs.get('task').sha256, 'e2806de4884f8cf91c7e859ed741607545e2062674658d3b45fb69a4480d5f4d');
  assert.equal(outputs.get('approval').sha256, 'ff2fe50c966a3f3dace2bc241bf604c42b6c84d75142f0a640e0b516724d20c7');
  assert.equal(outputs.get('reference').bytes, '45594c010001000800000002000000016104000000000000002a000000016200');
  assert.equal(outputs.get('approval').bytes, outputs.get('approval-execution').bytes);
  assert.equal(outputs.get('approval').sha256, outputs.get('approval-transport').sha256);
  assert.notEqual(outputs.get('approval').sha256, outputs.get('approval-effect-change').sha256);
  assert.equal(hex(await host.digest(new Uint8Array())), 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855');
  assert.equal(hex(await host.digest(new TextEncoder().encode('abc'))), 'ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad');
  const sample = new Uint8Array(readFileSync(join(temp, 'task.bin')));
  const retained = host.process(sample);
  for (let i = 0; i < 500; i++) assert.equal(hex(host.process(sample)), hex(retained));
  const e = instance.exports;
  const aliasedPointer = e.eyl_allocate_input(sample.length);
  const aliased = new Uint8Array(e.memory.buffer, aliasedPointer, sample.length);
  aliased.set(sample);
  assert.equal(hex(host.process(aliased)), outputs.get('task').bytes, 'input is copied before allocation clears aliased memory');
  assert.equal(e.eyl_allocate_input(0), 0); assert.equal(e.eyl_output_len(), 0);
  assert.equal(e.eyl_allocate_input(MAX_WIRE_BYTES + 1), 0); assert.equal(e.eyl_process(1, 1), 1);
  assert.ok(e.eyl_allocate_input(17)); assert.equal(e.eyl_process(16, 3), 1); assert.equal(e.eyl_output_len(), 0);
  assert.equal(e.eyl_process(17, 99), 2); assert.equal(e.eyl_output_len(), 0);
  assert.throws(() => host.process(sample, 99));
  assert.throws(() => host.randomId(7)); assert.throws(() => host.process(new Uint8Array(MAX_WIRE_BYTES + 1)));
  const unavailable = codecHost(instance, null);
  await assert.rejects(() => unavailable.digest(sample), /unavailable/);
  assert.throws(() => unavailable.randomId(0), /unavailable/);
  const failure = codecHost(instance, {subtle: {digest: async () => { throw new Error('provider failed'); }}, getRandomValues: () => { throw new Error('provider failed'); }});
  await assert.rejects(() => failure.digest(sample), /provider failed/); assert.throws(() => failure.randomId(0), /provider failed/);
  const first = host.randomId(0), second = host.randomId(0);
  assert.match(first, /^task:[0-9a-f]{32}$/); assert.match(second, /^task:[0-9a-f]{32}$/); assert.notEqual(first, second);
  assert.equal(hex(retained), outputs.get('task').bytes, 'copied output survives subsequent mutation');
  console.log(JSON.stringify({status:'passed', node:process.version, host:'macOS ARM64 / Node WebCrypto', accepted, rejected, repeated:500,
    task_sha256:outputs.get('task').sha256, approval_sha256:outputs.get('approval').sha256,
    boundary:'Actual Node-hosted Wasm execution; browser execution, executor and GPU/model acceptance not_run'}, null, 2));
} finally { rmSync(temp, {recursive:true, force:true}); }
