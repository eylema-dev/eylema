import assert from 'node:assert/strict';
import test from 'node:test';
import {
  initializeWebGpuScene,
  handleDeviceLoss,
  moveObject,
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

test('pointer selection maps the bounded canvas into three scene objects', () => {
  assert.equal(selectObjectAt(0, 300), 0);
  assert.equal(selectObjectAt(100, 300), 1);
  assert.equal(selectObjectAt(299, 300), 2);
  assert.equal(selectObjectAt(400, 300), 2);
  assert.equal(selectObjectAt(0, 0), null);
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
