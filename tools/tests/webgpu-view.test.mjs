import assert from 'node:assert/strict';
import test from 'node:test';
import {
  initializeWebGpuScene,
  handleDeviceLoss,
  moveObject,
  markReadyIfActive,
  requestWebGpuAdapter,
  selectObjectAt,
  watchDeviceLoss,
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

test('pointer selection follows projected cube positions after movement', () => {
  const positions = Array.from({ length: 3 }, () => ({ x: 0, y: 0 }));
  assert.equal(selectObjectAt(75, 300, positions), 0);
  assert.equal(selectObjectAt(150, 300, positions), 1);
  assert.equal(selectObjectAt(225, 300, positions), 2);
  positions[0] = moveObject(positions[0], 'right');
  positions[0] = moveObject(positions[0], 'right');
  positions[0] = moveObject(positions[0], 'right');
  const movedCenter = (Math.cos(0.48) * (-1.35 + positions[0].x) * 1.35 / (-Math.sin(0.48) * (-1.35 + positions[0].x) + 4.2) + 1) * 150;
  assert.equal(selectObjectAt(movedCenter, 300, positions), 0);
  assert.equal(selectObjectAt(0, 0, positions), null);
});

test('keyboard movement is bounded and rejects unknown directions', () => {
  assert.deepEqual(moveObject({ x: 0, y: 0 }, 'right'), { x: 0.16, y: 0 });
  assert.deepEqual(moveObject({ x: 0.64, y: 0 }, 'right'), { x: 0.64, y: 0 });
  assert.deepEqual(moveObject({ x: 0, y: -0.64 }, 'down'), { x: 0, y: -0.64 });
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
