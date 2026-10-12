import assert from 'node:assert/strict';
import test from 'node:test';
import {
  initializeWebGpuScene,
  handleDeviceLoss,
  moveObject,
  markReadyIfActive,
  projectObjectBounds,
  requestWebGpuAdapter,
  selectObjectAt,
  watchDeviceLoss,
  measureSceneFrames,
} from '../../crates/eylema-web/src/webgpu-view.mjs';

function fakeDocument() {
  const elements = new Map();
  for (const id of ['scene-canvas', 'scene-status', 'scene-selection', 'scene-select-next', 'scene-move-left', 'scene-move-right', 'scene-move-up', 'scene-move-down']) {
    elements.set(id, { disabled: false, textContent: '', addEventListener() {} });
  }
  return { documentElement: { lang: 'en' }, getElementById: (id) => elements.get(id), elements };
}

test('missing or unavailable adapter is explicit and disables every control', async () => {
  assert.equal(await requestWebGpuAdapter(undefined), null);
  assert.equal(await requestWebGpuAdapter({ requestAdapter: async () => null }), null);
  const documentRef = fakeDocument();
  const scene = await initializeWebGpuScene({ gpu: null, documentRef });
  assert.equal(scene.state, 'unsupported');
  assert.match(documentRef.elements.get('scene-status').textContent, /UNSUPPORTED/);
  for (const [id, control] of documentRef.elements) if (id.startsWith('scene-') && id !== 'scene-status' && id !== 'scene-selection' && id !== 'scene-canvas') assert.equal(control.disabled, true);
});

test('measurement bounds prevent arbitrary allocation and preserve unavailable memory as null', async () => {
  const base = { ready: () => true, render() {}, completed: async () => {}, nextFrame: async () => 10,
    now: () => 1, wallNow: () => 100, memory: () => null };
  for (const frameCount of [0, 1, 241, 1.5, NaN]) {
    await assert.rejects(measureSceneFrames({ ...base, frameCount }), RangeError);
  }
  const times = [10, 27, 48];
  const samples = await measureSceneFrames({ ...base, frameCount: 3, nextFrame: async () => times.shift() });
  assert.deepEqual(samples.map((sample) => sample.raf_interval_ms), [null, 17, 21]);
  assert.deepEqual(samples.map((sample) => sample.js_heap_bytes), [null, null, null]);
});

test('measurement rejects device loss, decreasing clocks and invalid memory rather than returning success', async () => {
  const base = { frameCount: 2, ready: () => true, render() {}, completed: async () => {},
    nextFrame: async () => 10, now: () => 1, wallNow: () => 100, memory: () => null };
  await assert.rejects(measureSceneFrames({ ...base, ready: () => false }), /GPU_UNAVAILABLE/);
  let active = true;
  await assert.rejects(measureSceneFrames({ ...base, ready: () => active,
    completed: async () => { active = false; } }), /GPU_UNAVAILABLE/);
  const times = [10, 9];
  await assert.rejects(measureSceneFrames({ ...base, nextFrame: async () => times.shift() }), /INVALID_FRAME_CLOCK/);
  const completion = [10, 9];
  await assert.rejects(measureSceneFrames({ ...base, now: () => completion.shift() }), /INVALID_COMPLETION_CLOCK/);
  await assert.rejects(measureSceneFrames({ ...base, memory: () => NaN }), /INVALID_MEMORY_SAMPLE/);
});

test('failed frame retains preceding raw samples for the evaluation report', async () => {
  let completions = 0;
  await assert.rejects(measureSceneFrames({ frameCount: 3, ready: () => true, render() {},
    nextFrame: async () => 10, now: () => 1, wallNow: () => 100, memory: () => null,
    completed: async () => { if (++completions === 2) throw new Error('device fixture'); },
  }), (error) => {
    assert.equal(error.samples.length, 1);
    assert.equal(error.failed_frame_index, 1);
    assert.equal(error.samples[0].index, 0);
    return true;
  });
});

test('pointer selection follows projected cube bounds on both axes and stays unambiguous at movement limits', () => {
  const positions = Array.from({ length: 3 }, () => ({ x: 0, y: 0 }));
  const center = (box) => ({ x: (box.minX + box.maxX + 2) * 75, y: 100 - (box.minY + box.maxY) * 50 });
  let bounds = projectObjectBounds(positions);
  for (const index of [0, 1, 2]) {
    const point = center(bounds[index]);
    assert.equal(selectObjectAt(point.x, point.y, 300, 200, positions), index);
    assert.equal(selectObjectAt(point.x, 1, 300, 200, positions), null);
  }
  positions[0] = moveObject(positions[0], 'right');
  positions[1] = moveObject(positions[1], 'left');
  bounds = projectObjectBounds(positions);
  assert.ok(bounds[0].maxX < bounds[1].minX, 'maximum opposing movement keeps adjacent projected cubes separate');
  for (const index of [0, 1]) {
    const point = center(bounds[index]);
    assert.equal(selectObjectAt(point.x, point.y, 300, 200, positions), index);
  }
  assert.equal(selectObjectAt(0, 0, 300, 200, positions), null);
});

test('keyboard movement is bounded and rejects unknown directions', () => {
  assert.deepEqual(moveObject({ x: 0, y: 0 }, 'right'), { x: 0.16, y: 0 });
  assert.deepEqual(moveObject({ x: 0.16, y: 0 }, 'right'), { x: 0.16, y: 0 });
  assert.deepEqual(moveObject({ x: 0, y: -0.16 }, 'down'), { x: 0, y: -0.16 });
  assert.throws(() => moveObject({ x: 0, y: 0 }, 'forward'), RangeError);
});

test('device-loss fixture reports the device reason through the production observer', async () => {
  let resolveLost;
  const device = { lost: new Promise((resolve) => { resolveLost = resolve; }) };
  const status = { textContent: '' };
  const controls = { one: { disabled: false }, two: { disabled: false } };
  let released = false;
  const watching = watchDeviceLoss(device, (info) => handleDeviceLoss(info, {
    status, controls, language: 'en', release: () => { released = true; },
  }));
  const fixture = { reason: 'destroyed', message: 'simulated device loss' };
  resolveLost(fixture);
  await watching;
  assert.match(status.textContent, /DEVICE_LOST \(destroyed\)/);
  assert.match(status.textContent, /Reload the page/);
  assert.equal(controls.one.disabled, true);
  assert.equal(controls.two.disabled, true);
  assert.equal(released, true);
});

test('device loss during asynchronous setup cannot restore ready controls', async () => {
  let finishSetup;
  const setup = new Promise((resolve) => { finishSetup = resolve; });
  let state = 'starting';
  let activated = false;
  const initialization = setup.then(() => markReadyIfActive(state, () => { activated = true; state = 'ready'; }));
  state = 'lost';
  finishSetup();
  assert.equal(await initialization, false);
  assert.equal(state, 'lost');
  assert.equal(activated, false);
});
