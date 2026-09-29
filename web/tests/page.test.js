// Tests of the page files that need no browser: the strings, the version, the security rules
// of the page and the Content-Security-Policy.
// Run with: node --test web/tests/

import assert from 'node:assert/strict';
import { readFileSync, readdirSync } from 'node:fs';
import test from 'node:test';

import { STRINGS } from '../strings.js';
import { VERSION } from '../version.js';

const web = new URL('../', import.meta.url);
const root = new URL('../../', import.meta.url);
const read = (path, base = web) => readFileSync(new URL(path, base), 'utf8');

const pageScripts = readdirSync(web).filter((name) => name.endsWith('.js'));
const html = read('index.html');
const app = read('app.js');

test('English and Korean have the same keys and no empty string', () => {
  assert.deepEqual(Object.keys(STRINGS.ko).sort(), Object.keys(STRINGS.en).sort());
  for (const table of [STRINGS.en, STRINGS.ko]) {
    for (const [key, value] of Object.entries(table)) {
      assert.equal(typeof value, 'string', key);
      assert.notEqual(value.trim(), '', key);
    }
  }
  // Placeholders are the same in both languages.
  const names = (text) => [...text.matchAll(/\{([a-z_]+)\}/g)].map((m) => m[1]).sort();
  for (const key of Object.keys(STRINGS.en)) {
    assert.deepEqual(names(STRINGS.ko[key]), names(STRINGS.en[key]), key);
  }
});

test('every string key the page names exists', () => {
  const used = new Set();
  for (const match of html.matchAll(/data-i18n(?:-aria-label|-placeholder)?="([a-z0-9_]+)"/g)) used.add(match[1]);
  for (const match of app.matchAll(/\bt\('([a-z0-9_]+)'/g)) used.add(match[1]);
  for (const match of app.matchAll(/\b(?:setReadStatus|t)\(\s*'([a-z0-9_]+)'/g)) used.add(match[1]);
  for (const match of app.matchAll(/'((?:make|read|camera|url|file|record|notice|text|bytes|engine)_[a-z0-9_]+)'/g)) {
    used.add(match[1]);
  }
  assert.ok(used.size > 40, `found only ${used.size} keys`);
  for (const key of used) assert.ok(Object.hasOwn(STRINGS.en, key), `missing string ${key}`);
});

test('every reader error of the specification has a sentence', () => {
  // The names are read from the Rust source that the reader reports them from.
  const source = read('crates/nmtcode-core/src/error.rs', root);
  const names = new Set([...source.matchAll(/=> "(E_[A-Z0-9_]+)"/g)].map((m) => m[1]));
  assert.equal(names.size, 29);
  const keys = new Set(Object.keys(STRINGS.en).filter((k) => k.startsWith('err_')).map((k) => k.slice(4)));
  assert.deepEqual([...keys].sort(), [...names].sort());
});

test('every error code of the encoder has a sentence', () => {
  const source = read('crates/nmtcode-wasm/src/make.rs', root);
  const block = source.slice(source.indexOf('pub const fn code'));
  const codes = [...block.slice(0, block.indexOf('\n    }\n')).matchAll(/=> "([a-z0-9_]+)"/g)].map((m) => m[1]);
  assert.equal(codes.length, 8);
  for (const code of codes) assert.ok(Object.hasOwn(STRINGS.en, `make_error_${code}`), code);
});

test('the actions that name a scheme have a confirmation', () => {
  for (const scheme of ['mailto', 'tel', 'sms', 'geo']) assert.ok(Object.hasOwn(STRINGS.en, `confirm_${scheme}`), scheme);
});

test('the page version is the workspace version', () => {
  const manifest = read('Cargo.toml', root);
  const section = manifest.slice(manifest.indexOf('[workspace.package]'));
  const match = section.match(/^version\s*=\s*"([^"]+)"/m);
  assert.ok(match);
  assert.equal(VERSION, match[1]);
  const [major] = VERSION.split('.').map(Number);
  assert.equal(major, 0, 'no 1.0.0 without an explicit decision');
});

test('no page script writes HTML or evaluates code', () => {
  const forbidden = [
    /\.innerHTML\b/,
    /\.outerHTML\b/,
    /insertAdjacentHTML/,
    /document\.write/,
    /\beval\s*\(/,
    /new\s+Function\b/,
    /setTimeout\(\s*['"`]/,
    /setAttribute\(\s*['"](?:on\w+|style|href|src)['"]/,
  ];
  for (const name of pageScripts) {
    const source = read(name);
    for (const pattern of forbidden) assert.doesNotMatch(source, pattern, `${name}: ${pattern}`);
  }
});

test('the page fetches nothing from outside its folder', () => {
  for (const name of pageScripts) {
    const source = read(name);
    assert.doesNotMatch(source, /\bfetch\s*\(/, name);
    assert.doesNotMatch(source, /XMLHttpRequest|WebSocket|EventSource|sendBeacon/, name);
    for (const match of source.matchAll(/(?:import\s[^'"]*from\s*|import\s*\(\s*)['"]([^'"]+)['"]/g)) {
      assert.match(match[1], /^\.\//, `${name} imports ${match[1]}`);
    }
  }
  for (const match of html.matchAll(/(?:src|href)="([^"]+)"/g)) {
    const target = match[1];
    // The only absolute address is a link that the user may follow; nothing loads from it.
    if (target === 'https://github.com/needmoretruth/nmtcode') continue;
    assert.doesNotMatch(target, /^[a-z][a-z0-9+.-]*:|^\/\//i, target);
  }
});

test('the Content-Security-Policy is the one decided', () => {
  const match = html.match(/<meta http-equiv="Content-Security-Policy" content="([^"]+)">/);
  assert.ok(match);
  assert.equal(
    match[1],
    "default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; worker-src 'self'; img-src 'self' blob: data:; media-src 'self' blob:; style-src 'self'; connect-src 'self'; object-src 'none'; base-uri 'none'",
  );
  assert.doesNotMatch(html, /\sstyle="/, 'inline styles are blocked by style-src');
  assert.doesNotMatch(html, /<script(?![^>]*\ssrc=)[^>]*>/, 'inline scripts are blocked by script-src');
  assert.doesNotMatch(html, /\son[a-z]+="/, 'inline handlers are blocked by script-src');
});

test('the footer carries the trademark sentence of specification 8.7', () => {
  assert.equal(STRINGS.en.footer_trademark, 'QR Code is a registered trademark of DENSO WAVE INCORPORATED.');
  assert.match(html, /data-i18n="footer_trademark"/);
  assert.match(html, /href="THIRD-PARTY-LICENSES\.html"/);
  assert.match(html, /href="https:\/\/github\.com\/needmoretruth\/nmtcode"/);
});

test('the camera and file inputs are as specified', () => {
  assert.match(html, /<input type="file" id="image-input" accept="image\/\*" capture="environment" hidden>/);
  assert.match(app, /facingMode: \{ ideal: 'environment' \}/);
  assert.match(app, /width: \{ ideal: 1920 \}/);
  assert.match(app, /height: \{ ideal: 1080 \}/);
});
