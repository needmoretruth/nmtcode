// Tests of the presentation rules of specification 3.4.3 in web/present.js.
// Run with: node --test web/tests/

import assert from 'node:assert/strict';
import { domainToASCII, domainToUnicode } from 'node:url';
import test from 'node:test';

import {
  ACTION_SCHEMES,
  NEVER_OPENED,
  escapeCodePoint,
  escapeHidden,
  fallbackFileName,
  fileExtension,
  fill,
  hexLines,
  hostToUnicode,
  isDeviceName,
  isHiddenCodePoint,
  limitCodePoints,
  openRule,
  pickLanguage,
  presentUrl,
  punycodeDecode,
  safeFileName,
} from '../present.js';

/** Every code point that 3.4.3 lists as hidden, from the ranges written in the specification. */
function specHiddenList() {
  const list = [];
  const range = (from, to) => {
    for (let cp = from; cp <= to; cp += 1) list.push(cp);
  };
  range(0x0000, 0x001f);
  range(0x007f, 0x009f);
  list.push(0x061c, 0x200e, 0x200f);
  range(0x202a, 0x202e);
  range(0x2066, 0x2069);
  list.push(0x00ad, 0x180e);
  range(0x200b, 0x200d);
  range(0x2060, 0x2064);
  list.push(0xfeff);
  return new Set(list);
}

test('the hidden characters are exactly the list of 3.4.3', () => {
  const expected = specHiddenList();
  for (let cp = 0; cp <= 0x10ffff; cp += 1) {
    if (cp >= 0xd800 && cp <= 0xdfff) continue;
    assert.equal(isHiddenCodePoint(cp, false), expected.has(cp), `U+${cp.toString(16)}`);
  }
  // Types 1 and 3 keep line feed and tab, and only those two.
  for (const cp of expected) {
    assert.equal(isHiddenCodePoint(cp, true), cp !== 0x0a && cp !== 0x09, `U+${cp.toString(16)}`);
  }
});

test('hidden characters are shown as \\u{XXXX} escapes', () => {
  assert.equal(escapeCodePoint(0x202e), '\\u{202E}');
  assert.equal(escapeCodePoint(0x7), '\\u{0007}');
  assert.equal(escapeHidden('invoice\u202Efdp.exe'), 'invoice\\u{202E}fdp.exe');
  assert.equal(escapeHidden('pay\u200Bpal.com'), 'pay\\u{200B}pal.com');
  assert.equal(escapeHidden('a\u001b[2Jb'), 'a\\u{001B}[2Jb');
  assert.equal(escapeHidden('\uFEFFstart\u00ADsoft'), '\\u{FEFF}start\\u{00AD}soft');
  assert.equal(escapeHidden('line\nnext\ttab'), 'line\\u{000A}next\\u{0009}tab');
  assert.equal(escapeHidden('line\nnext\ttab\r', { keepLineBreaks: true }), 'line\nnext\ttab\\u{000D}');
  assert.equal(escapeHidden('안녕 😀 é'), '안녕 😀 é');
});

test('long text is cut at a code point, with the counts', () => {
  assert.deepEqual(limitCodePoints('abc', 5), { text: 'abc', truncated: false, shown: 3, total: 3 });
  assert.deepEqual(limitCodePoints('abcdef', 3), { text: 'abc', truncated: true, shown: 3, total: 6 });
  assert.deepEqual(limitCodePoints('😀😀😀', 2), { text: '😀😀', truncated: true, shown: 2, total: 3 });
  assert.deepEqual(limitCodePoints('', 0), { text: '', truncated: false, shown: 0, total: 0 });
});

test('the scheme table of URL rule 5', () => {
  assert.equal(openRule('https'), 'open');
  assert.equal(openRule('http'), 'open_unencrypted');
  for (const scheme of ['mailto', 'tel', 'sms', 'geo']) assert.equal(openRule(scheme), 'confirm_action');
  for (const scheme of ['javascript', 'data', 'file', 'blob', 'vbscript', 'intent', 'content']) {
    assert.equal(openRule(scheme), 'never', scheme);
  }
  for (const scheme of ['ftp', 'bitcoin', 'otpauth', 'wifi', 'ssh', 'market', 'x-custom']) {
    assert.equal(openRule(scheme), 'confirm_scheme', scheme);
  }
  assert.deepEqual([...NEVER_OPENED].sort(), ['blob', 'content', 'data', 'file', 'intent', 'javascript', 'vbscript']);
  assert.deepEqual([...ACTION_SCHEMES].sort(), ['geo', 'mailto', 'sms', 'tel']);
});

test('URLs are parsed with the WHATWG parser before the scheme is judged', () => {
  // Case, leading spaces and tabs or line feeds inside the scheme do not hide javascript:.
  for (const value of ['javascript:alert(1)', 'JavaScript:alert(1)', ' javascript:alert(1)', 'java\nscript:alert(1)', 'java\tscript:x']) {
    const url = presentUrl(value);
    assert.equal(url.parsed, true, value);
    assert.equal(url.scheme, 'javascript', value);
    assert.equal(url.rule, 'never', value);
  }
  assert.equal(presentUrl('DATA:text/html,<b>x</b>').rule, 'never');
  assert.equal(presentUrl('intent://scan/#Intent;scheme=zxing;end').rule, 'never');
  assert.equal(presentUrl('HTTPS://Example.COM/a').rule, 'open');
  assert.equal(presentUrl('HTTPS://Example.COM/a').host, 'example.com');
  assert.equal(presentUrl('http://example.com/').rule, 'open_unencrypted');
  assert.equal(presentUrl('mailto:someone@example.com').rule, 'confirm_action');
  assert.equal(presentUrl('mailto:someone@example.com').host, null);
  assert.equal(presentUrl('bitcoin:bc1qexample?amount=1').rule, 'confirm_scheme');
});

test('a value the parser rejects is text that is never opened', () => {
  for (const value of ['not a url', 'example.com', 'https://', 'https://exa mple.com/', 'http://[::1']) {
    const url = presentUrl(value);
    assert.equal(url.parsed, false, value);
    assert.equal(url.rule, 'never', value);
    assert.equal(url.href, '', value);
  }
  assert.equal(presentUrl('example.com\u202E').text, 'example.com\\u{202E}');
});

test('the complete text is shown with every hidden character escaped, line feed and tab included', () => {
  const url = presentUrl('https://example.com/\u202Egpj.exe\n');
  assert.equal(url.text, 'https://example.com/\\u{202E}gpj.exe\\u{000A}');
  assert.equal(url.parsed, true);
  // What is opened is the parsed URL, not the raw text.
  assert.equal(url.href, 'https://example.com/%E2%80%AEgpj.exe');
});

test('the host is on its own line, with the A-label when it is an IDN', () => {
  const plain = presentUrl('https://github.com/needmoretruth/nmtcode');
  assert.equal(plain.host, 'github.com');
  assert.equal(plain.aLabel, null);

  const korean = presentUrl('https://한국.kr/경로');
  assert.equal(korean.host, '한국.kr');
  assert.equal(korean.aLabel, 'xn--3e0b707e.kr');

  // A Cyrillic look-alike of "apple": the A-label shows that it is not ASCII.
  const lookalike = presentUrl('https://аррӏе.com/');
  assert.equal(lookalike.host, 'аррӏе.com');
  assert.equal(lookalike.aLabel, 'xn--80ak6aa92e.com');

  // Written as an A-label, the Unicode form is shown next to it.
  const written = presentUrl('https://xn--80ak6aa92e.com/');
  assert.equal(written.host, 'аррӏе.com');
  assert.equal(written.aLabel, 'xn--80ak6aa92e.com');

  assert.equal(presentUrl('http://192.168.0.1:8080/').host, '192.168.0.1');
  assert.equal(presentUrl('http://[::1]/').host, '[::1]');
});

test('a user name or password before @ is flagged', () => {
  const tricky = presentUrl('https://paypal.com@evil.example/login');
  assert.equal(tricky.hasUserinfo, true);
  assert.equal(tricky.host, 'evil.example');
  assert.equal(presentUrl('https://user:pass@example.com/').hasUserinfo, true);
  assert.equal(presentUrl('https://:pass@example.com/').hasUserinfo, true);
  assert.equal(presentUrl('https://example.com/@user').hasUserinfo, false);
  assert.equal(presentUrl('mailto:a@example.com').hasUserinfo, false);
});

test('Punycode decoding agrees with the URL library of Node.js', () => {
  assert.equal(punycodeDecode('bcher-kva'), 'bücher');
  assert.equal(punycodeDecode('3e0b707e'), '한국');
  assert.equal(punycodeDecode('mnchen-3ya'), 'münchen');
  // RFC 3492, section 7.1, sample (A): Arabic (Egyptian).
  assert.equal(
    punycodeDecode('egbpdaj6bu4bxfgehfvwxn'),
    '\u0644\u064A\u0647\u0645\u0627\u0628\u062A\u0643\u0644\u0645\u0648\u0634\u0639\u0631\u0628\u064A\u061F',
  );
  for (const domain of ['münchen.de', '한국.kr', 'аррӏе.com', 'bücher.example', '日本語.jp', 'δοκιμή.gr', 'ÿ.test']) {
    const ascii = domainToASCII(domain);
    assert.ok(ascii.includes('xn--'), ascii);
    assert.equal(hostToUnicode(ascii), domainToUnicode(ascii), ascii);
  }
  assert.equal(punycodeDecode('!!'), null);
  assert.equal(punycodeDecode('99999999999'), null);
  assert.equal(hostToUnicode('xn--!!.com'), 'xn--!!.com');
  assert.equal(hostToUnicode('example.com'), 'example.com');
});

test('file names follow the six rules of 3.4.3', () => {
  // The examples of the specification.
  assert.equal(safeFileName('invoice\u202Efdp.exe'), 'invoicefdp.exe');
  assert.equal(safeFileName('../.bashrc'), 'bashrc');
  // 1. the part after the last / or \
  assert.equal(safeFileName('a/b/c.txt'), 'c.txt');
  assert.equal(safeFileName('C:\\Users\\a\\photo.jpg'), 'photo.jpg');
  assert.equal(safeFileName('dir/'), '');
  // 2. hidden characters removed, control characters included
  assert.equal(safeFileName('bell\u0007\nname\u0000.txt'), 'bellname.txt');
  assert.equal(safeFileName('re\u200Bport\uFEFF.pdf'), 'report.pdf');
  // 3. reserved characters replaced
  assert.equal(safeFileName('a:b*c?d"e<f>g|h'), 'a_b_c_d_e_f_g_h');
  // 4. leading dots, then trailing dots and spaces
  assert.equal(safeFileName('...hidden'), 'hidden');
  assert.equal(safeFileName('name. . .'), 'name');
  assert.equal(safeFileName('  spaced  '), '  spaced');
  assert.equal(safeFileName('..'), '');
  // 5. Windows device names, in any case, before the first dot
  assert.equal(safeFileName('con.txt'), '_con.txt');
  assert.equal(safeFileName('CON'), '_CON');
  assert.equal(safeFileName('Lpt9.tar.gz'), '_Lpt9.tar.gz');
  assert.equal(safeFileName('COM0'), '_COM0');
  assert.equal(safeFileName('console.log'), 'console.log');
  assert.equal(safeFileName('COM10'), 'COM10');
  assert.equal(isDeviceName('aux'), true);
  assert.equal(isDeviceName('\u0131'), false);
  // 6. at most 255 bytes of UTF-8, cut at a character boundary
  const long = 'é'.repeat(200);
  const cut = safeFileName(long);
  assert.equal(new TextEncoder().encode(cut).length, 254);
  assert.ok(long.startsWith(cut));
  assert.equal(new TextEncoder().encode(safeFileName('😀'.repeat(100))).length, 252);
  assert.equal(safeFileName('a'.repeat(300)).length, 255);
  // Applying the rules again changes nothing.
  for (const name of ['invoice\u202Efdp.exe', '../.bashrc', 'con.txt', 'a:b', 'x. . .', long]) {
    assert.equal(safeFileName(safeFileName(name)), safeFileName(name));
  }
});

test('the extension is shown apart from the name', () => {
  assert.equal(fileExtension('report.pdf'), '.pdf');
  assert.equal(fileExtension('archive.tar.gz'), '.gz');
  assert.equal(fileExtension('invoicefdp.exe'), '.exe');
  assert.equal(fileExtension('README'), '');
  assert.equal(fileExtension('name.'), '');
});

test('names the page chooses', () => {
  assert.equal(fallbackFileName('file', 4, 2), 'nmtcode-2.bin');
  assert.equal(fallbackFileName('psbt', 6, 1), 'nmtcode-1.psbt');
  assert.equal(fallbackFileName('text', 1, 1), 'nmtcode-1.txt');
  assert.equal(fallbackFileName('text', 3, 1), 'nmtcode-1.json');
  assert.equal(fallbackFileName('url', 2, 1), 'nmtcode-1.txt');
  assert.equal(fallbackFileName('unknown', 99, 3), 'nmtcode-3.bin');
});

test('bytes are shown as hexadecimal, 16 to a line', () => {
  const bytes = Uint8Array.from({ length: 20 }, (_, i) => i * 13);
  const hex = hexLines(bytes, 18);
  assert.equal(hex.text, '00 0D 1A 27 34 41 4E 5B 68 75 82 8F 9C A9 B6 C3\nD0 DD');
  assert.deepEqual([hex.truncated, hex.shown, hex.total], [true, 18, 20]);
  assert.equal(hexLines(new Uint8Array(0), 10).text, '');
});

test('string templates and the language choice', () => {
  assert.equal(fill('{a} of {b}', { a: 1, b: 'two' }), '1 of two');
  assert.equal(fill('{missing}', {}), '{missing}');
  assert.equal(fill('{a}', { a: '{b}', b: 'x' }), '{b}');
  assert.equal(pickLanguage(['ko-KR', 'en']), 'ko');
  assert.equal(pickLanguage(['ko']), 'ko');
  assert.equal(pickLanguage(['en-US', 'ko']), 'en');
  assert.equal(pickLanguage(['kok']), 'en');
  assert.equal(pickLanguage([]), 'en');
  assert.equal(pickLanguage(undefined), 'en');
});
