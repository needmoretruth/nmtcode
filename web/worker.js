// A Web Worker with its own instance of the NMT Code WebAssembly module. The page runs one for
// reading camera frames and image files and one for making codes, so neither blocks the page
// or the other.
//
// Messages in:
//   { id, op: 'decode', width, height, rgba: ArrayBuffer }
//   { id, op: 'encode', kind, data: ArrayBuffer, fileName, level, modulePx, bootstrap }
// Messages out:
//   { id, ok: true, reading }  reading = the JSON of decode_rgba with `bytes` on every record
//   { id, ok: true, made }     made = the fields of Encoded, `png` and `preview` as Uint8Array
//   { id, ok: false, error }   error = a code of the encoder, or 'engine' when the module
//                              could not load or failed

import init, { decode_rgba, encode } from './pkg/nmtcode_wasm.js';

const ready = init();

function decode(message) {
  const reading = decode_rgba(message.width, message.height, new Uint8Array(message.rgba));
  try {
    const result = JSON.parse(reading.json);
    const transfer = [];
    result.symbols.forEach((symbol, s) => {
      if (!symbol.ok) return;
      symbol.records.forEach((record, r) => {
        const bytes = reading.value(s, r);
        record.bytes = bytes;
        transfer.push(bytes.buffer);
      });
    });
    return { payload: { id: message.id, ok: true, reading: result }, transfer };
  } finally {
    reading.free();
  }
}

function make(message) {
  let encoded;
  try {
    encoded = encode(
      message.kind,
      new Uint8Array(message.data),
      message.fileName,
      message.level,
      message.modulePx,
      message.bootstrap,
    );
  } catch (error) {
    const code = error instanceof Error ? error.message : 'internal';
    return { payload: { id: message.id, ok: false, error: code }, transfer: [] };
  }
  try {
    const made = {
      png: encoded.png,
      svg: encoded.svg,
      preview: encoded.preview,
      previewWidth: encoded.previewWidth,
      previewHeight: encoded.previewHeight,
      imageWidth: encoded.imageWidth,
      imageHeight: encoded.imageHeight,
      width: encoded.width,
      height: encoded.height,
      level: encoded.level,
      codec: encoded.codec,
      dictionary: encoded.dictionary,
      bytesUsed: encoded.bytesUsed,
      capacity: encoded.capacity,
    };
    return {
      payload: { id: message.id, ok: true, made },
      transfer: [made.png.buffer, made.preview.buffer],
    };
  } finally {
    encoded.free();
  }
}

self.addEventListener('message', async (event) => {
  const message = event.data;
  try {
    await ready;
  } catch {
    self.postMessage({ id: message.id, ok: false, error: 'engine' });
    return;
  }
  try {
    const { payload, transfer } = message.op === 'decode' ? decode(message) : make(message);
    self.postMessage(payload, transfer);
  } catch {
    // A trap inside the module (not expected: the Rust code does not panic on input). The
    // page starts a new worker.
    self.postMessage({ id: message.id, ok: false, error: 'engine' });
  }
});
