const words = {
  en: {
    starting: 'Starting WebGPU…', unsupported: 'WebGPU unavailable: UNSUPPORTED. This browser or adapter cannot run the scene.',
    ready: 'WebGPU ready. Click a cube to select it; use arrow keys or the buttons to move it.',
    lost: (reason) => `WebGPU device lost: DEVICE_LOST (${reason || 'unknown'}). Reload the page to request a new device.`,
    failed: 'WebGPU initialization failed: GPU_UNAVAILABLE.', selected: (index) => `Selected object: ${index + 1}`,
    selectNext: 'Select next object', moveLeft: 'Move left', moveRight: 'Move right', moveUp: 'Move up', moveDown: 'Move down',
  },
  tr: {
    starting: 'WebGPU başlatılıyor…', unsupported: 'WebGPU kullanılamıyor: UNSUPPORTED. Bu tarayıcı veya bağdaştırıcı sahneyi çalıştıramıyor.',
    ready: 'WebGPU hazır. Seçmek için bir küpe tıklayın; ok tuşlarıyla veya düğmelerle taşıyın.',
    lost: (reason) => `WebGPU aygıtı kayboldu: DEVICE_LOST (${reason || 'bilinmiyor'}). Yeni bir aygıt istemek için sayfayı yenileyin.`,
    failed: 'WebGPU başlatılamadı: GPU_UNAVAILABLE.', selected: (index) => `Seçili nesne: ${index + 1}`,
    selectNext: 'Sonraki nesneyi seç', moveLeft: 'Sola taşı', moveRight: 'Sağa taşı', moveUp: 'Yukarı taşı', moveDown: 'Aşağı taşı',
  },
};
const OBJECT_SPACING = 1.7;
const CUBE_RADIUS = 0.38;

export function requestWebGpuAdapter(gpu) {
  if (!gpu || typeof gpu.requestAdapter !== 'function') return Promise.resolve(null);
  return gpu.requestAdapter();
}

export function projectObjectBounds(positions) {
  if (!Array.isArray(positions) || positions.length !== 3) return null;
  const corners = [-CUBE_RADIUS, CUBE_RADIUS];
  return positions.map((position, index) => {
    const projected = [];
    for (const x of corners) for (const y of corners) for (const z of corners) {
      const worldX = x + (index - 1) * OBJECT_SPACING + position.x;
      const worldY = y + position.y;
      const rotatedX = Math.cos(0.48) * worldX + Math.sin(0.48) * z;
      const rotatedZ = -Math.sin(0.48) * worldX + Math.cos(0.48) * z;
      const depth = rotatedZ + 4.2;
      projected.push({ x: rotatedX * 1.35 / depth, y: worldY * 1.35 / depth });
    }
    return {
      minX: Math.min(...projected.map((point) => point.x)), maxX: Math.max(...projected.map((point) => point.x)),
      minY: Math.min(...projected.map((point) => point.y)), maxY: Math.max(...projected.map((point) => point.y)),
    };
  });
}

export function selectObjectAt(x, y, width, height, positions = Array.from({ length: 3 }, () => ({ x: 0, y: 0 }))) {
  if (![x, y, width, height].every(Number.isFinite) || width <= 0 || height <= 0) return null;
  const clipX = (x / width) * 2 - 1;
  const clipY = 1 - (y / height) * 2;
  const bounds = projectObjectBounds(positions);
  if (!bounds) return null;
  const candidates = bounds.map((box, index) => ({ box, index })).filter(({ box }) =>
    clipX >= box.minX && clipX <= box.maxX && clipY >= box.minY && clipY <= box.maxY);
  if (candidates.length === 0) return null;
  const center = (box) => ({ x: (box.minX + box.maxX) / 2, y: (box.minY + box.maxY) / 2 });
  return candidates.reduce((closest, candidate) => {
    const a = center(candidate.box);
    const b = center(closest.box);
    return Math.hypot(a.x - clipX, a.y - clipY) < Math.hypot(b.x - clipX, b.y - clipY) ? candidate : closest;
  }).index;
}

export function markReadyIfActive(currentState, activate) {
  if (currentState !== 'starting') return false;
  activate();
  return true;
}

export function moveObject(position, direction, step = 0.16, limit = 0.16) {
  const delta = { left: [-step, 0], right: [step, 0], up: [0, step], down: [0, -step] }[direction];
  if (!delta || !Number.isFinite(step) || step <= 0 || !Number.isFinite(limit) || limit < step) {
    throw new RangeError('invalid scene movement');
  }
  return {
    x: Math.max(-limit, Math.min(limit, position.x + delta[0])),
    y: Math.max(-limit, Math.min(limit, position.y + delta[1])),
  };
}

export function watchDeviceLoss(device, onLost) {
  if (!device?.lost || typeof device.lost.then !== 'function') throw new TypeError('GPUDevice.lost promise required');
  return device.lost.then(onLost);
}

export function handleDeviceLoss(info, { status, controls, language = 'en', release = () => {} }) {
  status.textContent = words[language === 'tr' ? 'tr' : 'en'].lost(info?.reason);
  setControlsDisabled(controls, true);
  release();
}

// Evaluation host clocks measure animation cadence and queue-observed wall
// latency. They are not GPU timestamp queries or presented-frame timestamps.
export async function measureSceneFrames({
  frameCount, ready, render, completed,
  nextFrame = () => new Promise((resolve) => globalThis.requestAnimationFrame(resolve)),
  now = () => globalThis.performance.now(),
  wallNow = () => Date.now(),
  memory = () => globalThis.performance?.memory?.usedJSHeapSize ?? null,
}) {
  if (!Number.isInteger(frameCount) || frameCount < 2 || frameCount > 240) {
    throw new RangeError('frame count must be between 2 and 240');
  }
  const samples = [];
  let previous = null;
  try {
    for (let index = 0; index < frameCount; index += 1) {
      if (!ready()) throw new Error('GPU_UNAVAILABLE');
      const timestamp = await nextFrame();
      if (!ready()) throw new Error('GPU_UNAVAILABLE');
      const interval = previous === null ? null : timestamp - previous;
      if (!Number.isFinite(timestamp) || (interval !== null && (!Number.isFinite(interval) || interval < 0))) {
        throw new Error('INVALID_FRAME_CLOCK');
      }
      previous = timestamp;
      const start = now();
      const startUnix = wallNow();
      render(index);
      await completed();
      if (!ready()) throw new Error('GPU_UNAVAILABLE');
      const elapsed = now() - start;
      if (!Number.isFinite(elapsed) || elapsed < 0) throw new Error('INVALID_COMPLETION_CLOCK');
      const heap = memory();
      if (heap !== null && (!Number.isFinite(heap) || heap < 0)) throw new Error('INVALID_MEMORY_SAMPLE');
      samples.push({ index, start_unix_ms: startUnix, end_unix_ms: wallNow(), raf_interval_ms: interval,
        queue_completion_ms: elapsed, js_heap_bytes: heap });
    }
  } catch (error) {
    error.samples = samples;
    error.failed_frame_index = samples.length;
    throw error;
  }
  return samples;
}

function cubeVertices() {
  const faces = [
    { points: [[-1,-1,1],[1,-1,1],[1,1,1],[-1,1,1]], color: [0.25,0.78,0.68] },
    { points: [[1,-1,-1],[-1,-1,-1],[-1,1,-1],[1,1,-1]], color: [0.30,0.58,0.92] },
    { points: [[-1,1,1],[1,1,1],[1,1,-1],[-1,1,-1]], color: [0.96,0.66,0.28] },
    { points: [[-1,-1,-1],[1,-1,-1],[1,-1,1],[-1,-1,1]], color: [0.84,0.38,0.38] },
    { points: [[1,-1,1],[1,-1,-1],[1,1,-1],[1,1,1]], color: [0.66,0.48,0.88] },
    { points: [[-1,-1,-1],[-1,-1,1],[-1,1,1],[-1,1,-1]], color: [0.35,0.76,0.42] },
  ];
  const triangles = [0,1,2,0,2,3];
  const vertices = [];
  for (let object = 0; object < 3; object += 1) {
    for (const face of faces) for (const index of triangles) vertices.push({ position: face.points[index].map((v) => v * 0.38), color: face.color, object });
  }
  const stride = 28;
  const bytes = new ArrayBuffer(vertices.length * stride);
  const view = new DataView(bytes);
  vertices.forEach((vertex, index) => {
    const base = index * stride;
    vertex.position.forEach((value, axis) => view.setFloat32(base + axis * 4, value, true));
    vertex.color.forEach((value, channel) => view.setFloat32(base + 12 + channel * 4, value, true));
    view.setUint32(base + 24, vertex.object, true);
  });
  return { bytes, count: vertices.length, stride };
}

function setControlsDisabled(controls, disabled) {
  Object.values(controls).forEach((control) => { control.disabled = disabled; });
}

export async function initializeWebGpuScene({
  gpu = globalThis.navigator?.gpu,
  documentRef = globalThis.document,
  fetchImpl = globalThis.fetch,
} = {}) {
  const canvas = documentRef?.getElementById('scene-canvas');
  const status = documentRef?.getElementById('scene-status');
  const selection = documentRef?.getElementById('scene-selection');
  const controls = {
    next: documentRef?.getElementById('scene-select-next'), left: documentRef?.getElementById('scene-move-left'),
    right: documentRef?.getElementById('scene-move-right'), up: documentRef?.getElementById('scene-move-up'),
    down: documentRef?.getElementById('scene-move-down'),
  };
  if (!canvas || !status || !selection || Object.values(controls).some((control) => !control)) {
    return { setLanguage() {}, state: 'unavailable' };
  }
  let language = documentRef.documentElement.lang === 'tr' ? 'tr' : 'en';
  let device;
  let buffers = [];
  let selected = 1;
  const positions = Array.from({ length: 3 }, () => ({ x: 0, y: 0 }));
  let state = 'starting';
  let released = false;
  let lossReason = 'unknown';
  const measurementOutput = documentRef.getElementById('scene-measurement');
  const parameters = new URLSearchParams(globalThis.location?.hash?.slice(1) || '');
  const measurementRequested = parameters.has('g22');
  const writeMeasurement = (value) => {
    if (measurementRequested && measurementOutput) {
      measurementOutput.hidden = false;
      measurementOutput.textContent = JSON.stringify(value);
    }
  };
  writeMeasurement({ status: 'pending', format_version: 1 });
  const updateSelection = () => { selection.textContent = words[language].selected(selected); };
  const setState = (next) => {
    state = next;
    status.textContent = next === 'ready' ? words[language].ready : next === 'lost' ? words[language].lost(lossReason) : next === 'unsupported' ? words[language].unsupported : next === 'failed' ? words[language].failed : words[language].starting;
    setControlsDisabled(controls, next !== 'ready');
    if (['unsupported', 'failed', 'lost'].includes(next)) writeMeasurement({ status: 'failed', reason: next });
  };
  const setLanguage = (next) => {
    language = next === 'tr' ? 'tr' : 'en';
    setState(state);
    updateSelection();
    controls.next.textContent = words[language].selectNext;
    controls.left.textContent = words[language].moveLeft;
    controls.right.textContent = words[language].moveRight;
    controls.up.textContent = words[language].moveUp;
    controls.down.textContent = words[language].moveDown;
  };
  const release = () => {
    released = true;
    buffers.forEach((buffer) => { try { buffer.destroy(); } catch (_) {} });
    buffers = [];
    if (device) { try { device.destroy(); } catch (_) {} }
  };
  const controller = { setLanguage, get state() { return state; }, get selected() { return selected; } };
  setLanguage(language);

  let adapter;
  try { adapter = await requestWebGpuAdapter(gpu); }
  catch (_) { setState('failed'); return controller; }
  if (!adapter) { setState('unsupported'); return controller; }
  try {
    device = await adapter.requestDevice();
    watchDeviceLoss(device, (info) => {
      lossReason = info?.reason || 'unknown';
      state = 'lost';
      handleDeviceLoss(info, { status, controls, language, release });
    });
    if (state === 'lost') return controller;
    const context = canvas.getContext('webgpu');
    if (!context) { setState('unsupported'); release(); return controller; }
    const response = await fetchImpl('/scene.wgsl', { credentials: 'same-origin' });
    if (state === 'lost') return controller;
    if (!response.ok) throw new Error('shader fetch failed');
    const shaderCode = await response.text();
    if (state === 'lost') return controller;
    const shader = device.createShaderModule({ code: shaderCode });
    const info = await shader.getCompilationInfo();
    if (state === 'lost') return controller;
    if (info.messages.some((message) => message.type === 'error')) throw new Error('WGSL compilation failed');
    const format = gpu.getPreferredCanvasFormat();
    context.configure({ device, format, alphaMode: 'opaque' });
    const pipeline = await device.createRenderPipelineAsync({
      layout: 'auto', vertex: {
        module: shader, entryPoint: 'vertex_main',
        buffers: [{ arrayStride: 28, attributes: [
          { shaderLocation: 0, offset: 0, format: 'float32x3' },
          { shaderLocation: 1, offset: 12, format: 'float32x3' },
          { shaderLocation: 2, offset: 24, format: 'uint32' },
        ] }],
      },
      fragment: { module: shader, entryPoint: 'fragment_main', targets: [{ format }] },
      primitive: { topology: 'triangle-list', cullMode: 'none' },
      depthStencil: { format: 'depth24plus', depthWriteEnabled: true, depthCompare: 'less' },
    });
    if (state === 'lost') return controller;
    const geometry = cubeVertices();
    const vertexBuffer = device.createBuffer({ size: geometry.bytes.byteLength, usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST });
    const uniformBuffer = device.createBuffer({ size: 16, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST });
    const depthTexture = device.createTexture({ size: [canvas.width, canvas.height, 1], format: 'depth24plus', usage: GPUTextureUsage.RENDER_ATTACHMENT });
    buffers = [vertexBuffer, uniformBuffer, depthTexture];
    device.queue.writeBuffer(vertexBuffer, 0, geometry.bytes);
    const bindGroup = device.createBindGroup({ layout: pipeline.getBindGroupLayout(0), entries: [{ binding: 0, resource: { buffer: uniformBuffer } }] });
    const render = () => {
      if (state !== 'ready') return;
      const uniform = new ArrayBuffer(16);
      const uniformView = new DataView(uniform);
      uniformView.setUint32(0, selected, true);
      uniformView.setFloat32(4, positions[selected].x, true);
      uniformView.setFloat32(8, positions[selected].y, true);
      device.queue.writeBuffer(uniformBuffer, 0, uniform);
      const encoder = device.createCommandEncoder();
      const pass = encoder.beginRenderPass({
        colorAttachments: [{ view: context.getCurrentTexture().createView(), clearValue: { r: 0.09, g: 0.15, b: 0.18, a: 1 }, loadOp: 'clear', storeOp: 'store' }],
        depthStencilAttachment: { view: depthTexture.createView(), depthClearValue: 1, depthLoadOp: 'clear', depthStoreOp: 'store' },
      });
      pass.setPipeline(pipeline);
      pass.setVertexBuffer(0, vertexBuffer);
      pass.setBindGroup(0, bindGroup);
      pass.draw(geometry.count);
      pass.end();
      device.queue.submit([encoder.finish()]);
    };
    const select = (index) => { selected = Math.max(0, Math.min(2, index)); updateSelection(); render(); };
    const move = (direction) => { positions[selected] = moveObject(positions[selected], direction); render(); };
    canvas.addEventListener('click', (event) => {
      const rect = canvas.getBoundingClientRect();
      select(selectObjectAt(event.clientX - rect.left, event.clientY - rect.top, rect.width, rect.height, positions) ?? selected);
      canvas.focus();
    });
    canvas.addEventListener('keydown', (event) => {
      const direction = { ArrowLeft: 'left', ArrowRight: 'right', ArrowUp: 'up', ArrowDown: 'down' }[event.key];
      if (direction) { event.preventDefault(); move(direction); }
      else if (event.key === 'Enter' || event.key === ' ') { event.preventDefault(); select((selected + 1) % 3); }
    });
    controls.next.addEventListener('click', () => select((selected + 1) % 3));
    for (const direction of ['left', 'right', 'up', 'down']) controls[direction].addEventListener('click', () => move(direction));
    globalThis.addEventListener?.('pagehide', release, { once: true });
    markReadyIfActive(state, () => {
      setState('ready');
      updateSelection();
      render();
    });
    if (measurementRequested && measurementOutput) {
      const frameCount = Number(parameters.get('g22'));
      const trial = Number(parameters.get('trial'));
      if (!Number.isInteger(trial) || trial < 1 || trial > 3) throw new RangeError('invalid measurement trial');
      const samples = await measureSceneFrames({
        frameCount, ready: () => state === 'ready' && !released,
        render: (index) => { positions[selected].x = 0.16 * Math.sin(index / 10); render(); },
        completed: () => device.queue.onSubmittedWorkDone(),
      });
      writeMeasurement({ format_version: 1, status: 'passed', trial, frame_count: frameCount,
        browser_user_agent: String(globalThis.navigator?.userAgent || 'unknown').slice(0, 512),
        adapter_info: Object.fromEntries(['vendor', 'architecture', 'device', 'description'].map((key) =>
          [key, String(adapter.info?.[key] || '').slice(0, 256)])),
        fallback_adapter: typeof adapter.isFallbackAdapter === 'boolean' ? adapter.isFallbackAdapter : null,
        canvas_extent: [canvas.width, canvas.height], declared_buffer_bytes: geometry.bytes.byteLength + 16,
        memory_scope: 'browser-reported JS heap estimate; excludes GPU/driver memory',
        latency_scope: 'animation-frame cadence and submit-to-queue-completion wall time; not GPU/presentation timestamps', samples });
    }
  } catch (error) {
    if (state !== 'lost') { setState('failed'); release(); }
    writeMeasurement({ format_version: 1, status: 'failed', reason: 'MEASUREMENT_OR_GPU_FAILURE',
      samples: Array.isArray(error?.samples) ? error.samples : [], failed_frame_index: error?.failed_frame_index ?? null });
  }
  return controller;
}
