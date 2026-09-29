// The NMT Code page: read codes with the camera or from an image file, and make codes.
// Decoded and typed content reaches the page only through textContent; nothing is sent
// anywhere. The WebAssembly module runs in Web Workers (worker.js).

import { STRINGS } from './strings.js';
import { VERSION } from './version.js';
import {
  escapeHidden,
  fallbackFileName,
  fileExtension,
  fill,
  hexLines,
  limitCodePoints,
  pickLanguage,
  presentUrl,
  safeFileName,
} from './present.js';

/** Characters of a text or link shown before "Show all" (3.4.3, "Size"). */
const TEXT_LIMIT = 5000;
/** Bytes shown as hexadecimal before the rest is offered as a file. */
const HEX_LIMIT = 256;
/** Least time between two camera frames: at most about 10 frames per second. */
const FRAME_INTERVAL_MS = 100;
/** Pause after the last change before a code is made. */
const ENCODE_DELAY_MS = 150;
/** The most decoded content one code holds (1 MiB, specification 6.4). */
const MAX_FILE_BYTES = 1_048_576;
/** Larger images are scaled down before they are read. */
const MAX_IMAGE_PIXELS = 40_000_000;
const MIN_MODULE_PX = 2;
const MAX_MODULE_PX = 16;
const LANGUAGE_KEY = 'nmtcode.language';

// ---------------------------------------------------------------------------------------------
// Language

function savedLanguage() {
  try {
    const saved = window.localStorage.getItem(LANGUAGE_KEY);
    if (saved === 'en' || saved === 'ko') return saved;
  } catch {
    // Storage may be unavailable; the browser's language is used.
  }
  return null;
}

let language =
  savedLanguage() ??
  pickLanguage(navigator.languages && navigator.languages.length > 0 ? navigator.languages : [navigator.language]);

/** The string `key` in the current language, with `{name}` values filled in. */
function t(key, values) {
  const template = STRINGS[language][key] ?? STRINGS.en[key] ?? key;
  return fill(template, values);
}

function number(value) {
  return new Intl.NumberFormat(language).format(value);
}

// ---------------------------------------------------------------------------------------------
// DOM helpers: every text goes through textContent.

function $(id) {
  const node = document.getElementById(id);
  if (!node) throw new Error(`missing #${id}`);
  return node;
}

/**
 * @param {string} tag
 * @param {{className?: string, text?: string, type?: string}} [options]
 * @param {(Node | null | undefined)[]} [children]
 */
function el(tag, { className, text, type } = {}, children = []) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (type) node.setAttribute('type', type);
  if (text !== undefined) node.textContent = text;
  for (const child of children) if (child) node.append(child);
  return node;
}

function button(text, kind, onClick) {
  const node = el('button', { className: `button ${kind}`, text, type: 'button' });
  node.addEventListener('click', onClick);
  return node;
}

function facts(rows) {
  const list = el('dl', { className: 'facts' });
  for (const [term, value] of rows) {
    list.append(el('dt', { text: term }), el('dd', { text: value }));
  }
  return list;
}

/** Saves bytes under `name` through a Blob URL; the file is never opened. */
function saveBytes(bytes, name, type = 'application/octet-stream') {
  const url = URL.createObjectURL(new Blob([bytes], { type }));
  const link = document.createElement('a');
  link.href = url;
  link.download = name;
  link.rel = 'noopener';
  document.body.append(link);
  link.click();
  link.remove();
  window.setTimeout(() => URL.revokeObjectURL(url), 60_000);
}

/** Opens a parsed URL after the user asked for it; http(s) in a new tab without a referrer. */
function openUrl(href, newTab) {
  const link = document.createElement('a');
  link.href = href;
  link.rel = 'noopener noreferrer';
  if (newTab) link.target = '_blank';
  document.body.append(link);
  link.click();
  link.remove();
}

// ---------------------------------------------------------------------------------------------
// Workers

class Engine {
  constructor() {
    this.worker = null;
    this.nextId = 1;
    this.pending = new Map();
  }

  ensure() {
    if (this.worker) return this.worker;
    const worker = new Worker(new URL('./worker.js', import.meta.url), { type: 'module' });
    worker.addEventListener('message', (event) => {
      const reply = event.data;
      const resolve = this.pending.get(reply.id);
      if (!resolve) return;
      this.pending.delete(reply.id);
      if (!reply.ok && reply.error === 'engine') this.reset();
      resolve(reply);
    });
    worker.addEventListener('error', (event) => {
      event.preventDefault();
      this.fail();
    });
    this.worker = worker;
    return worker;
  }

  reset() {
    this.worker?.terminate();
    this.worker = null;
  }

  fail() {
    this.reset();
    for (const [id, resolve] of this.pending) resolve({ id, ok: false, error: 'engine' });
    this.pending.clear();
  }

  /** Sends `message` and resolves with the reply; `transfer` lists buffers to move. */
  request(message, transfer = []) {
    const id = this.nextId;
    this.nextId += 1;
    return new Promise((resolve) => {
      this.pending.set(id, resolve);
      try {
        this.ensure().postMessage({ ...message, id }, transfer);
      } catch {
        this.fail();
      }
    });
  }
}

const reader = new Engine();
const maker = new Engine();

// ---------------------------------------------------------------------------------------------
// Views

const tabs = [$('tab-read'), $('tab-make')];

function selectView(name, focus) {
  for (const tab of tabs) {
    const selected = tab.id === `tab-${name}`;
    tab.setAttribute('aria-selected', String(selected));
    tab.tabIndex = selected ? 0 : -1;
    $(tab.getAttribute('aria-controls') ?? '').hidden = !selected;
    if (selected && focus) tab.focus();
  }
  if (name === 'make') {
    stopCamera();
    sizePreview();
  }
}

for (const tab of tabs) {
  tab.addEventListener('click', () => selectView(tab.id.replace('tab-', ''), false));
  tab.addEventListener('keydown', (event) => {
    const index = tabs.indexOf(tab);
    let next = -1;
    if (event.key === 'ArrowRight') next = (index + 1) % tabs.length;
    else if (event.key === 'ArrowLeft') next = (index + tabs.length - 1) % tabs.length;
    else if (event.key === 'Home') next = 0;
    else if (event.key === 'End') next = tabs.length - 1;
    if (next < 0) return;
    event.preventDefault();
    selectView(tabs[next].id.replace('tab-', ''), true);
  });
}

// ---------------------------------------------------------------------------------------------
// Read: camera and image files

const read = {
  stream: null,
  scanning: false,
  timer: 0,
  /** @type {{key: string, values?: object, tone: 'plain' | 'error'} | null} */
  status: null,
  /** The last reading with results: { symbols: [...] }. */
  reading: null,
  /** Records whose whole text is shown, as "symbol-record". */
  expanded: new Set(),
};

const video = /** @type {HTMLVideoElement} */ ($('video'));
const frameCanvas = document.createElement('canvas');
const frameContext = frameCanvas.getContext('2d', { willReadFrequently: true });

function setReadStatus(key, values, tone = 'plain') {
  read.status = key ? { key, values, tone } : null;
  renderReadStatus();
}

function renderReadStatus() {
  const node = $('read-status');
  const text = read.status ? t(read.status.key, read.status.values) : '';
  if (node.textContent !== text) node.textContent = text;
  node.classList.toggle('error', read.status?.tone === 'error');
}

function renderCameraButton() {
  $('camera-button').textContent = t(read.stream ? 'read_camera_stop' : 'read_camera_start');
}

function cameraErrorKey(error) {
  const name = error && typeof error === 'object' && 'name' in error ? error.name : '';
  if (name === 'NotAllowedError' || name === 'SecurityError') return 'camera_error_denied';
  if (name === 'NotFoundError' || name === 'OverconstrainedError') return 'camera_error_missing';
  return 'camera_error_other';
}

async function startCamera() {
  if (!window.isSecureContext || !navigator.mediaDevices?.getUserMedia) {
    setReadStatus('camera_error_insecure', undefined, 'error');
    return;
  }
  setReadStatus('read_status_starting');
  let stream;
  try {
    stream = await navigator.mediaDevices.getUserMedia({
      audio: false,
      video: {
        facingMode: { ideal: 'environment' },
        width: { ideal: 1920 },
        height: { ideal: 1080 },
      },
    });
  } catch (error) {
    setReadStatus(cameraErrorKey(error), undefined, 'error');
    return;
  }
  read.stream = stream;
  read.reading = null;
  read.expanded.clear();
  renderResults();
  video.srcObject = stream;
  $('camera').hidden = false;
  renderCameraButton();
  try {
    await video.play();
  } catch {
    // Autoplay of a muted inline video is allowed; frames are read once they arrive.
  }
  read.scanning = true;
  setReadStatus('read_status_scanning');
  scheduleFrame(0);
}

function stopCamera() {
  read.scanning = false;
  window.clearTimeout(read.timer);
  if (read.stream) {
    for (const track of read.stream.getTracks()) track.stop();
    read.stream = null;
    if (read.status?.key === 'read_status_scanning' || read.status?.key === 'read_status_seen') {
      setReadStatus(null);
    }
  }
  video.srcObject = null;
  $('camera').hidden = true;
  renderCameraButton();
}

function scheduleFrame(delay) {
  window.clearTimeout(read.timer);
  read.timer = window.setTimeout(scanFrame, delay);
}

/** A reading is final when a symbol was presented or failed for a reason other than damage. */
function isFinal(reading) {
  return reading.symbols.some((symbol) => symbol.ok || symbol.outcome !== 'damaged');
}

async function scanFrame() {
  if (!read.scanning || !frameContext) return;
  const started = performance.now();
  const width = video.videoWidth;
  const height = video.videoHeight;
  if (width === 0 || height === 0) {
    scheduleFrame(FRAME_INTERVAL_MS);
    return;
  }
  if (frameCanvas.width !== width || frameCanvas.height !== height) {
    frameCanvas.width = width;
    frameCanvas.height = height;
  }
  frameContext.drawImage(video, 0, 0, width, height);
  const image = frameContext.getImageData(0, 0, width, height);
  const reply = await reader.request(
    { op: 'decode', width, height, rgba: image.data.buffer },
    [image.data.buffer],
  );
  if (!read.scanning) return;
  if (!reply.ok) {
    stopCamera();
    setReadStatus('engine_error', undefined, 'error');
    return;
  }
  const reading = reply.reading;
  if (isFinal(reading)) {
    stopCamera();
    setReadStatus(null);
    showReading(reading);
    return;
  }
  const seen = reading.symbols.find((symbol) => !symbol.ok);
  if (seen) setReadStatus('read_status_seen', { name: seen.error });
  else setReadStatus('read_status_scanning');
  scheduleFrame(Math.max(0, FRAME_INTERVAL_MS - (performance.now() - started)));
}

async function readImageFile(file) {
  stopCamera();
  read.reading = null;
  read.expanded.clear();
  renderResults();
  setReadStatus('read_status_file');
  let bitmap;
  try {
    bitmap = await createImageBitmap(file);
  } catch {
    setReadStatus('read_image_error', undefined, 'error');
    return;
  }
  const scale = Math.min(1, Math.sqrt(MAX_IMAGE_PIXELS / (bitmap.width * bitmap.height)));
  const width = Math.max(1, Math.floor(bitmap.width * scale));
  const height = Math.max(1, Math.floor(bitmap.height * scale));
  const canvas = document.createElement('canvas');
  canvas.width = width;
  canvas.height = height;
  const context = canvas.getContext('2d', { willReadFrequently: true });
  if (!context) {
    bitmap.close();
    setReadStatus('read_image_error', undefined, 'error');
    return;
  }
  // Nearest-neighbour when the image is kept at its size, so module edges stay sharp.
  context.imageSmoothingEnabled = scale < 1;
  context.drawImage(bitmap, 0, 0, width, height);
  bitmap.close();
  const image = context.getImageData(0, 0, width, height);
  const reply = await reader.request(
    { op: 'decode', width, height, rgba: image.data.buffer },
    [image.data.buffer],
  );
  if (!reply.ok) {
    setReadStatus('engine_error', undefined, 'error');
    return;
  }
  if (reply.reading.symbols.length === 0) {
    setReadStatus('read_none_in_file');
    return;
  }
  setReadStatus(null);
  showReading(reply.reading);
}

function showReading(reading) {
  read.reading = reading;
  read.expanded.clear();
  renderResults();
}

// ---------------------------------------------------------------------------------------------
// Read: presenting the records (specification 3.4.3)

function errorSentence(name) {
  const key = `err_${name}`;
  return Object.prototype.hasOwnProperty.call(STRINGS.en, key) ? t(key) : t('error_unknown');
}

function errorLine(name, className) {
  return el('p', { className }, [el('span', { className: 'error-name', text: name }), document.createTextNode(` ${errorSentence(name)}`)]);
}

function renderResults() {
  const box = $('results');
  box.replaceChildren();
  const symbols = read.reading?.symbols ?? [];
  symbols.forEach((symbol, index) => box.append(renderSymbol(symbol, index, symbols.length)));
}

function renderSymbol(symbol, index, count) {
  const box = el('article', { className: 'box' });
  if (count > 1) box.append(el('h2', { text: t('result_symbol', { n: number(index + 1) }) }));
  if (!symbol.ok) {
    box.append(errorLine(symbol.error, 'error'));
    return box;
  }
  if (symbol.notice) box.append(errorLine(symbol.notice, 'notice record'));
  symbol.records.forEach((record, r) => {
    // A file name record is shown as the name of the file record after it.
    if (record.present_as === 'file_name') return;
    box.append(renderRecord(record, `${index}-${r}`, r + 1));
  });
  box.append(
    el('p', {
      className: 'meta result-meta',
      text: t('result_size', { width: number(symbol.width), height: number(symbol.height), level: symbol.level }),
    }),
  );
  return box;
}

function renderRecord(record, key, n) {
  switch (record.present_as) {
    case 'text':
      return renderText(record, key, n);
    case 'url':
      return renderUrl(record, key, n);
    case 'file':
    case 'psbt':
      return renderFile(record, n);
    case 'bytes':
      return renderBytes(record, n, t('record_bytes'), null);
    default:
      return renderBytes(record, n, t('record_unknown'), t('record_unknown_detail', { type: record.type }));
  }
}

function recordBlock(label) {
  return el('div', { className: 'record' }, [el('h3', { text: label })]);
}

/** Shows `value` in part or in full, with "Show all" and, when long, "Save as a file". */
function limitedValue(block, value, key, keepLineBreaks, record, n) {
  const expanded = read.expanded.has(key);
  const limited = limitCodePoints(value, TEXT_LIMIT);
  const shown = expanded ? value : limited.text;
  block.append(el('pre', { className: 'value', text: escapeHidden(shown, { keepLineBreaks }) }));
  if (!limited.truncated) return true;
  if (!expanded) {
    block.append(el('p', { className: 'meta', text: t('text_truncated', { shown: number(limited.shown), total: number(limited.total) }) }));
  }
  const actions = el('div', { className: 'actions' });
  if (!expanded) {
    actions.append(
      button(t('text_show_all'), 'secondary', () => {
        read.expanded.add(key);
        renderResults();
      }),
    );
  }
  const name = fallbackFileName(record.present_as, record.type, n);
  actions.append(button(t('save_as_file'), 'secondary', () => saveBytes(record.bytes, name)));
  block.append(actions);
  return expanded;
}

function renderText(record, key, n) {
  const block = recordBlock(t(record.type === 3 ? 'record_json' : 'record_text'));
  limitedValue(block, record.text ?? '', key, true, record, n);
  return block;
}

function renderUrl(record, key, n) {
  const block = recordBlock(t('record_url'));
  const value = record.text ?? '';
  const url = presentUrl(value);
  const complete = limitedValue(block, value, key, false, record, n);
  if (!url.parsed) {
    block.append(el('p', { className: 'notice', text: t('url_invalid') }));
    return block;
  }
  const rows = [];
  if (url.host !== null) rows.push([t('url_host'), url.host]);
  if (url.aLabel !== null) rows.push([t('url_alabel'), url.aLabel]);
  if (rows.length > 0) block.append(facts(rows));
  if (url.hasUserinfo) {
    block.append(el('p', { className: 'warning', text: t('url_userinfo', { host: url.host ?? '' }) }));
  }
  if (url.rule === 'never') {
    block.append(el('p', { className: 'notice', text: t('url_never', { scheme: escapeHidden(url.scheme) }) }));
    return block;
  }
  if (url.rule === 'open_unencrypted') {
    block.append(el('p', { className: 'warning', text: t('url_http') }));
  }
  if (!complete) {
    block.append(el('p', { className: 'notice', text: t('url_show_all_to_open') }));
    return block;
  }
  const area = el('div', { className: 'actions' });
  const openButton = button(t('url_open'), 'primary', () => {
    if (url.rule === 'open' || url.rule === 'open_unencrypted') {
      openUrl(url.href, true);
      return;
    }
    const question =
      url.rule === 'confirm_action' ? t(`confirm_${url.scheme}`) : t('confirm_scheme', { scheme: escapeHidden(url.scheme) });
    const panel = el('div', { className: 'confirm' }, [el('p', { text: question })]);
    const choices = el('div', { className: 'actions' });
    const cancel = button(t('confirm_cancel'), 'secondary', () => {
      panel.replaceWith(area);
      openButton.focus();
    });
    choices.append(
      button(t('confirm_continue'), 'primary', () => {
        openUrl(url.href, false);
        panel.replaceWith(area);
        openButton.focus();
      }),
      cancel,
    );
    panel.append(choices);
    area.replaceWith(panel);
    cancel.focus();
  });
  area.append(openButton);
  block.append(area);
  return block;
}

function renderFile(record, n) {
  const block = recordBlock(t(record.present_as === 'psbt' ? 'record_psbt' : 'record_file'));
  const safe = safeFileName(record.file_name ?? '');
  const name = safe !== '' ? safe : fallbackFileName(record.present_as, record.type, n);
  block.append(
    facts([
      [t('file_name'), name],
      [t('file_extension'), fileExtension(name) || t('file_extension_none')],
      [t('file_size'), t('record_length', { length: number(record.length) })],
    ]),
  );
  if (safe === '') block.append(el('p', { className: 'notice', text: t('file_name_none', { name }) }));
  if (record.notice === 'not_psbt') block.append(el('p', { className: 'notice', text: t('notice_not_psbt') }));
  const actions = el('div', { className: 'actions' });
  actions.append(button(t('file_save'), 'primary', () => saveBytes(record.bytes, name)));
  block.append(actions);
  return block;
}

function renderBytes(record, n, label, detail) {
  const block = recordBlock(label);
  if (detail) block.append(el('p', { className: 'notice', text: detail }));
  if (record.notice === 'not_utf8') block.append(el('p', { className: 'notice', text: t('notice_not_utf8') }));
  block.append(el('p', { className: 'meta', text: t('record_length', { length: number(record.length) }) }));
  const hex = hexLines(record.bytes, HEX_LIMIT);
  if (hex.text !== '') block.append(el('pre', { className: 'value hex', text: hex.text }));
  if (hex.truncated) {
    block.append(el('p', { className: 'meta', text: t('bytes_truncated', { shown: number(hex.shown), total: number(hex.total) }) }));
  }
  const safe = safeFileName(record.file_name ?? '');
  const name = safe !== '' ? safe : fallbackFileName(record.present_as, record.type, n);
  const actions = el('div', { className: 'actions' });
  actions.append(button(t('save_as_file'), 'secondary', () => saveBytes(record.bytes, name)));
  block.append(actions);
  return block;
}

$('camera-button').addEventListener('click', () => {
  if (read.stream) stopCamera();
  else startCamera();
});

const imageInput = /** @type {HTMLInputElement} */ ($('image-input'));
$('image-button').addEventListener('click', () => imageInput.click());
imageInput.addEventListener('change', () => {
  const file = imageInput.files?.[0];
  imageInput.value = '';
  if (file) readImageFile(file);
});

document.addEventListener('visibilitychange', () => {
  if (document.hidden && read.stream) stopCamera();
});

// ---------------------------------------------------------------------------------------------
// Make

const make = {
  kind: 'text',
  /** @type {{name: string, size: number, bytes: Uint8Array | null, error: string | null} | null} */
  file: null,
  requestId: 0,
  /** The last result, as the worker sent it. */
  result: null,
  /** A string key of this page that says why there is no code, or null. */
  error: null,
  /** A string key that says what to enter, while there is no content. */
  empty: 'make_empty_text',
  /** The result drawn on the preview canvas. */
  drawn: null,
  timer: 0,
};

const textInput = /** @type {HTMLTextAreaElement} */ ($('text-input'));
const urlInput = /** @type {HTMLInputElement} */ ($('url-input'));
const fileInput = /** @type {HTMLInputElement} */ ($('file-input'));
const levelSelect = /** @type {HTMLSelectElement} */ ($('level'));
const moduleRange = /** @type {HTMLInputElement} */ ($('module-range'));
const moduleNumber = /** @type {HTMLInputElement} */ ($('module-number'));
const qrCheckbox = /** @type {HTMLInputElement} */ ($('qr'));
const previewCanvas = /** @type {HTMLCanvasElement} */ ($('preview'));
const previewBox = $('preview-box');

function scheduleEncode() {
  window.clearTimeout(make.timer);
  make.timer = window.setTimeout(runEncode, ENCODE_DELAY_MS);
}

function moduleSize() {
  const value = Number(moduleNumber.value);
  return Number.isInteger(value) && value >= MIN_MODULE_PX && value <= MAX_MODULE_PX ? value : null;
}

function isValidUrl(value) {
  try {
    new URL(value);
    return true;
  } catch {
    return false;
  }
}

/** The content to encode, or a string key that says why there is none. */
function currentContent() {
  if (make.kind === 'text') {
    const text = textInput.value;
    return text === '' ? { empty: 'make_empty_text' } : { data: new TextEncoder().encode(text), fileName: '' };
  }
  if (make.kind === 'url') {
    const url = urlInput.value.trim();
    return url === '' ? { empty: 'make_empty_url' } : { data: new TextEncoder().encode(url), fileName: '' };
  }
  if (!make.file) return { empty: 'make_empty_file' };
  if (make.file.error) return { failed: make.file.error };
  if (!make.file.bytes) return { empty: 'make_empty_file' };
  return { data: make.file.bytes.slice(), fileName: make.file.name };
}

async function runEncode() {
  const content = currentContent();
  const id = make.requestId + 1;
  make.requestId = id;
  if (!content.data) {
    make.result = null;
    make.error = content.failed ?? null;
    make.empty = content.empty ?? null;
    renderMake();
    return;
  }
  const reply = await maker.request(
    {
      op: 'encode',
      kind: make.kind,
      data: content.data.buffer,
      fileName: content.fileName,
      level: Number(levelSelect.value),
      modulePx: moduleSize() ?? Number(moduleRange.value),
      bootstrap: qrCheckbox.checked,
    },
    [content.data.buffer],
  );
  // A newer request was sent while this one ran; its reply will be shown instead.
  if (id !== make.requestId) return;
  make.empty = null;
  make.result = reply.ok ? reply.made : null;
  make.error = reply.ok ? null : reply.error === 'engine' ? 'engine_error' : `make_error_${reply.error}`;
  renderMake();
}

function renderMake() {
  const hasResult = make.result !== null;
  $('make-empty').textContent = make.empty ? t(make.empty) : '';
  $('make-empty').hidden = !make.empty;
  const errorNode = $('make-error');
  const errorKey = make.error && Object.prototype.hasOwnProperty.call(STRINGS.en, make.error) ? make.error : make.error ? 'make_error_internal' : null;
  errorNode.textContent = errorKey ? t(errorKey) : '';
  errorNode.hidden = !errorKey;
  previewBox.hidden = !hasResult;
  $('download-png').toggleAttribute('disabled', !hasResult);
  $('download-svg').toggleAttribute('disabled', !hasResult);
  const info = $('make-info');
  if (!hasResult) {
    info.textContent = '';
    return;
  }
  const result = make.result;
  info.replaceChildren(
    document.createTextNode(
      t('make_info', {
        width: number(result.width),
        height: number(result.height),
        level: result.level,
        used: number(result.bytesUsed),
        capacity: number(result.capacity),
      }),
    ),
    el('br'),
    document.createTextNode(t('make_image_size', { width: number(result.imageWidth), height: number(result.imageHeight) })),
  );
  if (make.drawn !== result) drawPreview(result);
}

function drawPreview(result) {
  const context = previewCanvas.getContext('2d');
  if (!context) return;
  previewCanvas.width = result.previewWidth;
  previewCanvas.height = result.previewHeight;
  const image = context.createImageData(result.previewWidth, result.previewHeight);
  const pixels = image.data;
  result.preview.forEach((dark, i) => {
    const value = dark ? 0 : 255;
    pixels[4 * i] = value;
    pixels[4 * i + 1] = value;
    pixels[4 * i + 2] = value;
    pixels[4 * i + 3] = 255;
  });
  context.putImageData(image, 0, 0);
  make.drawn = result;
  sizePreview();
}

/**
 * Sizes the preview so that every module covers a whole number of device pixels
 * (specification 1.5, rendering rules), as large as fits the column and 60% of the window
 * height, and at least one device pixel per module.
 */
function sizePreview() {
  const columns = previewCanvas.width;
  const rows = previewCanvas.height;
  if (previewBox.hidden || columns === 0 || rows === 0) return;
  const style = window.getComputedStyle(previewBox);
  const width = previewBox.clientWidth - parseFloat(style.paddingLeft) - parseFloat(style.paddingRight);
  const height = window.innerHeight * 0.6;
  const ratio = window.devicePixelRatio || 1;
  const perModule = Math.max(1, Math.floor(Math.min((width * ratio) / columns, (height * ratio) / rows)));
  previewCanvas.style.width = `${(columns * perModule) / ratio}px`;
  previewCanvas.style.height = `${(rows * perModule) / ratio}px`;
}

new ResizeObserver(() => sizePreview()).observe(previewBox);
window.addEventListener('resize', () => sizePreview());

function selectKind(kind) {
  make.kind = kind;
  $('text-field').hidden = kind !== 'text';
  $('url-field').hidden = kind !== 'url';
  $('file-field').hidden = kind !== 'file';
  scheduleEncode();
}

for (const radio of document.querySelectorAll('input[name="kind"]')) {
  radio.addEventListener('change', () => {
    if (/** @type {HTMLInputElement} */ (radio).checked) selectKind(/** @type {HTMLInputElement} */ (radio).value);
  });
}

textInput.addEventListener('input', scheduleEncode);
urlInput.addEventListener('input', () => {
  const value = urlInput.value.trim();
  $('url-hint').hidden = value === '' || isValidUrl(value);
  scheduleEncode();
});
levelSelect.addEventListener('change', scheduleEncode);
qrCheckbox.addEventListener('change', scheduleEncode);
moduleRange.addEventListener('input', () => {
  moduleNumber.value = moduleRange.value;
  scheduleEncode();
});
moduleNumber.addEventListener('input', () => {
  const value = moduleSize();
  if (value === null) return;
  moduleRange.value = String(value);
  scheduleEncode();
});
moduleNumber.addEventListener('change', () => {
  let value = Math.round(Number(moduleNumber.value));
  if (!Number.isFinite(value)) value = Number(moduleRange.value);
  value = Math.min(MAX_MODULE_PX, Math.max(MIN_MODULE_PX, value));
  moduleNumber.value = String(value);
  moduleRange.value = String(value);
  scheduleEncode();
});

$('file-button').addEventListener('click', () => fileInput.click());
fileInput.addEventListener('change', async () => {
  const file = fileInput.files?.[0];
  fileInput.value = '';
  if (!file) return;
  make.file = { name: file.name, size: file.size, bytes: null, error: null };
  renderFileChosen();
  if (file.size > MAX_FILE_BYTES) {
    make.file.error = 'make_file_too_large';
  } else {
    try {
      make.file.bytes = new Uint8Array(await file.arrayBuffer());
    } catch {
      make.file.error = 'make_file_read_error';
    }
  }
  scheduleEncode();
});

function renderFileChosen() {
  $('file-chosen').textContent = make.file
    ? t('make_file_chosen', { name: escapeHidden(make.file.name), size: number(make.file.size) })
    : t('make_file_none');
}

$('download-png').addEventListener('click', () => {
  const result = make.result;
  if (result) saveBytes(result.png, `nmtcode-${result.width}x${result.height}.png`, 'image/png');
});
$('download-svg').addEventListener('click', () => {
  const result = make.result;
  if (result) saveBytes(new TextEncoder().encode(result.svg), `nmtcode-${result.width}x${result.height}.svg`, 'image/svg+xml');
});

// ---------------------------------------------------------------------------------------------
// Language and start

function applyLanguage() {
  document.documentElement.lang = language;
  document.title = t('page_title');
  for (const node of document.querySelectorAll('[data-i18n]')) {
    node.textContent = t(/** @type {HTMLElement} */ (node).dataset.i18n ?? '');
  }
  for (const node of document.querySelectorAll('[data-i18n-aria-label]')) {
    node.setAttribute('aria-label', t(/** @type {HTMLElement} */ (node).dataset.i18nAriaLabel ?? ''));
  }
  for (const node of document.querySelectorAll('[data-i18n-placeholder]')) {
    /** @type {HTMLInputElement} */ (node).placeholder = t(/** @type {HTMLElement} */ (node).dataset.i18nPlaceholder ?? '');
  }
  const other = language === 'en' ? 'ko' : 'en';
  const switcher = $('language-button');
  switcher.textContent = t(`language_${other}`);
  switcher.lang = other;
  $('top-version').textContent = VERSION;
  $('footer-version').textContent = t('footer_version', { version: VERSION });
  renderCameraButton();
  renderReadStatus();
  renderResults();
  renderFileChosen();
  renderMake();
}

$('language-button').addEventListener('click', () => {
  language = language === 'en' ? 'ko' : 'en';
  try {
    window.localStorage.setItem(LANGUAGE_KEY, language);
  } catch {
    // The choice then lasts until the page is closed.
  }
  applyLanguage();
});

// A reload may restore the form; start from what it shows.
moduleNumber.value = moduleRange.value;
$('url-hint').hidden = urlInput.value.trim() === '' || isValidUrl(urlInput.value.trim());
applyLanguage();
selectView('read', false);
const checkedKind = /** @type {HTMLInputElement | null} */ (document.querySelector('input[name="kind"]:checked'));
selectKind(checkedKind?.value ?? 'text');
