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

export function requestWebGpuAdapter(gpu) {
  if (!gpu || typeof gpu.requestAdapter !== 'function') return Promise.resolve(null);
  return gpu.requestAdapter();
}

export function selectObjectAt(x, width, count = 3) {
  if (!Number.isFinite(x) || !Number.isFinite(width) || width <= 0 || !Number.isInteger(count) || count < 1) return null;
  const clamped = Math.max(0, Math.min(width - Number.EPSILON, x));
  return Math.min(count - 1, Math.floor((clamped / width) * count));
}

export function moveObject(position, direction, step = 0.16, limit = 0.64) {
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
  let lossReason = 'unknown';
  const updateSelection = () => { selection.textContent = words[language].selected(selected); };
  const setState = (next) => {
    state = next;
    status.textContent = next === 'ready' ? words[language].ready : next === 'lost' ? words[language].lost(lossReason) : next === 'unsupported' ? words[language].unsupported : next === 'failed' ? words[language].failed : words[language].starting;
    setControlsDisabled(controls, next !== 'ready');
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
    const context = canvas.getContext('webgpu');
    if (!context) { setState('unsupported'); release(); return controller; }
    const response = await fetchImpl('/scene.wgsl', { credentials: 'same-origin' });
    if (!response.ok) throw new Error('shader fetch failed');
    const shader = device.createShaderModule({ code: await response.text() });
    const info = await shader.getCompilationInfo();
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
      select(selectObjectAt(event.clientX - rect.left, rect.width) ?? selected);
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
    setState('ready');
    updateSelection();
    render();
  } catch (_) {
    if (state !== 'lost') { setState('failed'); release(); }
  }
  return controller;
}
