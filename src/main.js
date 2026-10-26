const { invoke } = window.__TAURI__.core;

// [key, label, default, min, max, step, unit]
const GROUPS = [
  {
    title: 'Color channels',
    toggle: ['posterize', 'Posterize input image', true],
    fields: [
      ['red', 'Red', 7, 1, 20, 1, 'steps'],
      ['green', 'Green', 7, 1, 20, 1, 'steps'],
      ['blue', 'Blue', 7, 1, 20, 1, 'steps'],
      ['alpha', 'Alpha', 1, 1, 10, 1, 'steps'],
    ],
  },
  {
    title: 'Size and rotation',
    fields: [
      ['scale', 'Scale', 100, 1, 200, 1, '%'],
      ['rotation', 'Rotation', 0, 0, 360, 1, '°'],
    ],
  },
  {
    title: 'Image filters',
    fields: [
      ['brightness', 'Brightness', 100, 0, 200, 1, '%'],
      ['contrast', 'Contrast', 100, 0, 200, 1, '%'],
      ['grayscale', 'Grayscale', 0, 0, 100, 1, '%'],
      ['hueRotate', 'Hue rotate', 0, 0, 360, 1, '°'],
      ['invert', 'Invert', 0, 0, 100, 1, '%'],
      ['opacity', 'Opacity', 100, 0, 100, 1, '%'],
      ['saturate', 'Saturate', 100, 0, 200, 1, '%'],
      ['sepia', 'Sepia', 0, 0, 100, 1, '%'],
    ],
  },
  {
    title: 'SVG options',
    select: ['curves', 'Curves', 'spline', ['spline', 'polygon', 'pixel']],
    fields: [
      ['speckle', 'Suppress speckles', 2, 0, 50, 1, 'px'],
      ['cornerThreshold', 'Corner threshold', 60, 0, 180, 1, '°'],
      ['segmentLength', 'Segment length', 4, 3.5, 10, 0.5, 'px'],
      ['spliceThreshold', 'Splice threshold', 45, 0, 180, 1, '°'],
      ['strokeWidth', 'Stroke width', 0, 0, 10, 0.5, 'px'],
      ['minPathSegments', 'Min path segments', 0, 0, 30, 1, ''],
      ['precision', 'Decimal places', 2, 0, 4, 1, ''],
    ],
  },
];

const $ = (id) => document.getElementById(id);
const view = $('view');
const stage = $('stage');
const statusEl = $('status');

const defaults = { color: true };
for (const g of GROUPS) {
  if (g.toggle) defaults[g.toggle[0]] = g.toggle[2];
  if (g.select) defaults[g.select[0]] = g.select[2];
  for (const [key, , value] of g.fields) defaults[key] = value;
}

let settings = { ...defaults };
try {
  Object.assign(settings, JSON.parse(localStorage.getItem('settings')) ?? {});
} catch {}
const store = () => {
  try {
    localStorage.setItem('settings', JSON.stringify(settings));
  } catch {}
};

const status = (text) => (statusEl.textContent = text);
const sizeLabel = (n) => (n < 1024 ? `${n} B` : n < 1048576 ? `${(n / 1024).toFixed(1)} KB` : `${(n / 1048576).toFixed(2)} MB`);

// Controls ---------------------------------------------------------------

const inputs = {};
const sync = () => {
  for (const [key, el] of Object.entries(inputs)) {
    if (el.type === 'checkbox') el.checked = settings[key];
    else el.value = settings[key];
    el.dispatchEvent(new Event('sync'));
  }
  document.querySelector(`input[name=mode][value=${settings.color ? 'color' : 'mono'}]`).checked = true;
};

const onChange = (key, value) => {
  settings[key] = value;
  store();
  retrace();
};

for (const g of GROUPS) {
  const details = document.createElement('details');
  details.open = true;
  const summary = document.createElement('summary');
  summary.textContent = g.title;
  const reset = document.createElement('button');
  reset.className = 'reset';
  reset.textContent = 'Reset';
  reset.addEventListener('click', (e) => {
    e.preventDefault();
    for (const k of [g.toggle?.[0], g.select?.[0], ...g.fields.map((f) => f[0])]) {
      if (k) settings[k] = defaults[k];
    }
    sync();
    store();
    retrace();
  });
  summary.append(reset);
  details.append(summary);

  if (g.toggle) {
    const [key, label] = g.toggle;
    const row = document.createElement('label');
    row.className = 'check';
    const box = Object.assign(document.createElement('input'), { type: 'checkbox' });
    box.addEventListener('change', () => onChange(key, box.checked));
    // Channel sliders mean nothing while posterize is off.
    box.addEventListener('sync', () => details.querySelectorAll('input[type=range]').forEach((r) => (r.disabled = !box.checked)));
    box.addEventListener('change', () => box.dispatchEvent(new Event('sync')));
    row.append(box, ` ${label}`);
    details.append(row);
    inputs[key] = box;
  }

  if (g.select) {
    const [key, label, , options] = g.select;
    const row = document.createElement('label');
    row.className = 'field';
    const select = document.createElement('select');
    for (const o of options) select.append(new Option(o[0].toUpperCase() + o.slice(1), o));
    select.addEventListener('change', () => onChange(key, select.value));
    row.append(Object.assign(document.createElement('span'), { textContent: label }), select);
    details.append(row);
    inputs[key] = select;
  }

  for (const [key, label, , min, max, step, unit] of g.fields) {
    const row = document.createElement('label');
    row.className = 'field';
    const name = Object.assign(document.createElement('span'), { textContent: label });
    const value = Object.assign(document.createElement('span'), { className: 'value' });
    const range = Object.assign(document.createElement('input'), { type: 'range', min, max, step });
    const show = () => (value.textContent = `${range.value}${unit === 'steps' ? ' steps' : unit}`);
    range.addEventListener('input', () => {
      show();
      onChange(key, Number(range.value));
    });
    range.addEventListener('sync', show);
    row.append(name, value, range);
    details.append(row);
    inputs[key] = range;
  }
  $('groups').append(details);
}

for (const radio of document.querySelectorAll('input[name=mode]')) {
  radio.addEventListener('change', () => onChange('color', radio.value === 'color'));
}
sync();

// Tracing ----------------------------------------------------------------

let loaded = false;
let svg = '';
let fileName = 'image';
let busy = false;
let again = false;

// Latest wins: while one trace runs, slider moves collapse into a single follow-up.
async function retrace() {
  if (!loaded) return;
  if (busy) {
    again = true;
    return;
  }
  busy = true;
  stage.classList.add('busy');
  status('Tracing…');
  const t = performance.now();
  try {
    svg = await invoke('trace', { params: settings });
    view.innerHTML = svg;
    status(`${sizeLabel(svg.length)} · ${Math.round(performance.now() - t)} ms`);
    $('save').disabled = $('copy').disabled = false;
  } catch (e) {
    status(String(e));
  } finally {
    busy = false;
    stage.classList.remove('busy');
    if (again) {
      again = false;
      retrace();
    }
  }
}

async function load(blob, name = 'image') {
  status('Loading…');
  try {
    const [w, h] = await invoke('load_image', new Uint8Array(await blob.arrayBuffer()));
    fileName = name.replace(/\.[^.]+$/, '') || 'image';
    loaded = true;
    $('hint').hidden = true;
    resetView();
    document.title = `${fileName} (${w}×${h}) · svgKonijn`;
    retrace();
  } catch (e) {
    status(`Could not open image: ${e}`);
  }
}

$('open').addEventListener('click', () => $('file').click());
$('file').addEventListener('change', (e) => {
  const f = e.target.files[0];
  if (f) load(f, f.name);
  e.target.value = '';
});
window.addEventListener('dragover', (e) => e.preventDefault());
window.addEventListener('drop', (e) => {
  e.preventDefault();
  const f = [...e.dataTransfer.files].find((f) => f.type.startsWith('image/'));
  if (f) load(f, f.name);
});
window.addEventListener('paste', (e) => {
  const f = [...e.clipboardData.files].find((f) => f.type.startsWith('image/'));
  if (f) load(f, 'pasted');
});

$('save').addEventListener('click', async () => {
  try {
    if (await invoke('save_svg', { svg, name: fileName })) status('Saved');
  } catch (e) {
    status(`Save failed: ${e}`);
  }
});
$('copy').addEventListener('click', async () => {
  try {
    await invoke('copy_svg', { svg });
    status('SVG copied to clipboard');
  } catch (e) {
    status(`Copy failed: ${e}`);
  }
});

// Pan and zoom -------------------------------------------------------------

let zoom = 1;
let panX = 0;
let panY = 0;
const applyView = () => (view.style.transform = `translate(${panX}px, ${panY}px) scale(${zoom})`);
function resetView() {
  zoom = 1;
  panX = panY = 0;
  applyView();
}

stage.addEventListener(
  'wheel',
  (e) => {
    e.preventDefault();
    const r = stage.getBoundingClientRect();
    const k = Math.exp(-e.deltaY * 0.002);
    const mx = e.clientX - r.left;
    const my = e.clientY - r.top;
    panX = mx - (mx - panX) * k;
    panY = my - (my - panY) * k;
    zoom *= k;
    applyView();
  },
  { passive: false },
);
stage.addEventListener('pointerdown', (e) => stage.setPointerCapture(e.pointerId));
stage.addEventListener('pointermove', (e) => {
  if (!stage.hasPointerCapture(e.pointerId)) return;
  panX += e.movementX;
  panY += e.movementY;
  applyView();
});
stage.addEventListener('dblclick', resetView);
