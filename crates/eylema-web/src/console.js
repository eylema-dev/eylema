import { initializeWebGpuScene } from './webgpu-view.mjs';

(() => {
  const $ = (id) => document.getElementById(id);
  const copy = {
    en: { eyebrow: 'Local task console', intro: 'Create and run a bounded task on this Mac. Task and execution IDs stay in this browser so you can inspect them after a reload.', note: 'This session is not verified as a person. A stop request may be denied; the console will keep showing the observed state.', language: 'Language', actions: 'Task actions', delta: 'Value to add', create: 'Create task', run: 'Run task', inspect: 'Refresh status', stop: 'Request stop', state: 'Observed state', task: 'Task ID', execution: 'Execution ID', value: 'Current value', progress: 'Steps completed', executionState: 'Execution state', scene: 'WebGPU scene', sceneStatus: 'Starting WebGPU…', selection: 'Selected object: —', selectNext: 'Select next object', moveLeft: 'Move left', moveRight: 'Move right', moveUp: 'Move up', moveDown: 'Move down', ready: 'Ready.', created: 'Task created.', running: 'Task submitted.', stopRequested: 'Stop request accepted.', stopped: 'Stop request denied: FORBIDDEN. Observed state is unchanged.', failed: 'Request failed: ', empty: 'No task is saved in this browser.' },
    tr: { eyebrow: 'Yerel görev konsolu', intro: 'Bu Mac’te sınırlandırılmış bir görev oluşturup çalıştırın. Yeniden yüklemeden sonra incelemek için görev ve yürütme kimlikleri bu tarayıcıda tutulur.', note: 'Bu oturum bir kişi olarak doğrulanmamıştır. Durdurma isteği reddedilebilir; konsol gözlenen durumu göstermeyi sürdürür.', language: 'Dil', actions: 'Görev işlemleri', delta: 'Eklenecek değer', create: 'Görev oluştur', run: 'Görevi çalıştır', inspect: 'Durumu yenile', stop: 'Durdurma isteği', state: 'Gözlenen durum', task: 'Görev kimliği', execution: 'Yürütme kimliği', value: 'Geçerli değer', progress: 'Tamamlanan adımlar', executionState: 'Yürütme durumu', scene: 'WebGPU sahnesi', sceneStatus: 'WebGPU başlatılıyor…', selection: 'Seçili nesne: —', selectNext: 'Sonraki nesneyi seç', moveLeft: 'Sola taşı', moveRight: 'Sağa taşı', moveUp: 'Yukarı taşı', moveDown: 'Aşağı taşı', ready: 'Hazır.', created: 'Görev oluşturuldu.', running: 'Görev gönderildi.', stopRequested: 'Durdurma isteği kabul edildi.', stopped: 'Durdurma isteği reddedildi: FORBIDDEN. Gözlenen durum değişmedi.', failed: 'İstek başarısız: ', empty: 'Bu tarayıcıda kayıtlı görev yok.' }
  };
  const fields = ['eyebrow','intro','authority-note','language-label','actions-title','delta-label','create','run','inspect','stop','state-title','task-label','execution-label','value-label','progress-label','execution-state-label','scene-title','scene-status','scene-selection','scene-select-next','scene-move-left','scene-move-right','scene-move-up','scene-move-down'];
  const keys = ['eyebrow','intro','note','language','actions','delta','create','run','inspect','stop','state','task','execution','value','progress','executionState','scene','sceneStatus','selection','selectNext','moveLeft','moveRight','moveUp','moveDown'];
  const setText = (id, value) => { $(id).textContent = value; };
  let state = { task: '', execution: '', lang: 'en' };
  try { state = { ...state, ...JSON.parse(localStorage.getItem('eylema-console-state') || '{}') }; } catch (_) {}
  const save = () => localStorage.setItem('eylema-console-state', JSON.stringify(state));
  let setSceneLanguage = () => {};
  function language(lang) {
    state.lang = lang; save(); document.documentElement.lang = lang;
    fields.forEach((id, i) => setText(id, copy[lang][keys[i]]));
    setSceneLanguage(lang);
  }
  function render(data) {
    if (data.task) state.task = data.task;
    if (data.execution) state.execution = data.execution;
    save(); setText('task-id', state.task || '—'); setText('execution-id', state.execution || '—');
    if (data.value !== undefined) setText('value', String(data.value));
    if (data.steps !== undefined) setText('progress', String(data.steps));
    if (data.state) setText('execution-state', data.state);
    $('run').disabled = !state.task || !!state.execution; $('inspect').disabled = !state.task; $('stop').disabled = !state.execution;
  }
  async function call(path, payload = {}) {
    const csrf = document.querySelector('meta[name="eylema-csrf"]').content;
    const response = await fetch(`/console/v1/${path}`, { method: 'POST', credentials: 'same-origin', headers: { 'Content-Type': 'application/json', 'X-Eylema-CSRF': csrf }, body: JSON.stringify(payload) });
    const data = await response.json();
    if (!response.ok) { const e = new Error(data.error || 'REQUEST_FAILED'); e.code = data.error || 'REQUEST_FAILED'; throw e; }
    return data;
  }
  async function act(path, payload, success) {
    $('error').textContent = '';
    try { const data = await call(path, payload); render(data); $('message').textContent = copy[state.lang][success]; }
    catch (e) { $('error').textContent = copy[state.lang].failed + (e.code || e.message); if (e.code === 'FORBIDDEN') $('message').textContent = copy[state.lang].stopped; }
  }
  $('language').value = state.lang; language(state.lang);
  $('language').addEventListener('change', () => language($('language').value));
  $('create').addEventListener('click', async () => {
    $('error').textContent = '';
    try {
      const data = await call('create', { delta: Number($('delta').value) });
      state.task = data.task; state.execution = ''; render(data);
      $('message').textContent = copy[state.lang].created;
    } catch (e) {
      $('error').textContent = copy[state.lang].failed + (e.code || e.message);
      if (e.code === 'FORBIDDEN') $('message').textContent = copy[state.lang].stopped;
    }
  });
  $('run').addEventListener('click', () => act('run', { task: state.task, delta: Number($('delta').value) }, 'running'));
  $('inspect').addEventListener('click', () => act('inspect', { task: state.task, execution: state.execution }, 'ready'));
  $('stop').addEventListener('click', () => act('stop', { execution: state.execution }, 'stopRequested'));
  initializeWebGpuScene().then((scene) => { setSceneLanguage = scene.setLanguage; scene.setLanguage(state.lang); });
  if (state.task) act('inspect', { task: state.task, execution: state.execution }, 'ready'); else $('message').textContent = copy[state.lang].empty;
})();
