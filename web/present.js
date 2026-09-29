// Presentation rules of the NMT Code specification, chapter 3 (3.4.3), for the reader page.
// Pure functions without the DOM, so that `node --test web/tests/` can test them.

// ---------------------------------------------------------------------------------------------
// Hidden characters

/**
 * Whether a code point is a hidden character of 3.4.3: a C0 or C1 control character
 * (U+0000 to U+001F, U+007F to U+009F), a character that changes the direction of text
 * (U+061C, U+200E, U+200F, U+202A to U+202E, U+2066 to U+2069), or an invisible character
 * (U+00AD, U+180E, U+200B to U+200D, U+2060 to U+2064, U+FEFF).
 *
 * @param {number} cp the code point
 * @param {boolean} keepLineBreaks true for text and JSON (types 1 and 3), where line feed and
 *   tab are shown as themselves
 */
export function isHiddenCodePoint(cp, keepLineBreaks) {
  if (keepLineBreaks && (cp === 0x0a || cp === 0x09)) return false;
  if (cp <= 0x1f || (cp >= 0x7f && cp <= 0x9f)) return true;
  if (cp === 0x061c || cp === 0x200e || cp === 0x200f) return true;
  if ((cp >= 0x202a && cp <= 0x202e) || (cp >= 0x2066 && cp <= 0x2069)) return true;
  if (cp === 0x00ad || cp === 0x180e || cp === 0xfeff) return true;
  if ((cp >= 0x200b && cp <= 0x200d) || (cp >= 0x2060 && cp <= 0x2064)) return true;
  return false;
}

/** The visible escape of a code point, for example `\u{202E}`. */
export function escapeCodePoint(cp) {
  return `\\u{${cp.toString(16).toUpperCase().padStart(4, '0')}}`;
}

/**
 * `text` with every hidden character of 3.4.3 replaced by its visible escape.
 *
 * @param {string} text
 * @param {{keepLineBreaks?: boolean}} [options] keep line feed and tab (types 1 and 3 only)
 */
export function escapeHidden(text, { keepLineBreaks = false } = {}) {
  const parts = [];
  for (const ch of text) {
    const cp = ch.codePointAt(0) ?? 0;
    parts.push(isHiddenCodePoint(cp, keepLineBreaks) ? escapeCodePoint(cp) : ch);
  }
  return parts.join('');
}

/**
 * The first `max` code points of `text`. The reader shows a long value in part and offers
 * the whole of it (3.4.3, "Size"); nothing is dropped silently.
 *
 * @returns {{text: string, truncated: boolean, shown: number, total: number}}
 */
export function limitCodePoints(text, max) {
  let count = 0;
  let end = 0;
  for (const ch of text) {
    if (count === max) {
      let total = count;
      for (const _ of text.slice(end)) total += 1;
      return { text: text.slice(0, end), truncated: true, shown: count, total };
    }
    count += 1;
    end += ch.length;
  }
  return { text, truncated: false, shown: count, total: count };
}

// ---------------------------------------------------------------------------------------------
// URL rules (type 2)

/** Schemes that are never offered to open (3.4.3, URL rule 5). */
export const NEVER_OPENED = Object.freeze([
  'javascript',
  'data',
  'file',
  'blob',
  'vbscript',
  'intent',
  'content',
]);

/** Schemes that are offered with a confirmation that names the action (URL rule 5). */
export const ACTION_SCHEMES = Object.freeze(['mailto', 'tel', 'sms', 'geo']);

/**
 * How a URL of `scheme` may be opened (3.4.3, URL rule 5), always after a user action and
 * after the complete text has been shown:
 *
 * - `open`: `https`, opened after the tap;
 * - `open_unencrypted`: `http`, opened after the tap, with a notice that the connection is
 *   not encrypted;
 * - `confirm_action`: `mailto`, `tel`, `sms`, `geo`, after a confirmation that names the action;
 * - `never`: the schemes of {@link NEVER_OPENED};
 * - `confirm_scheme`: any other scheme, after a confirmation that names the scheme.
 *
 * @param {string} scheme lower case, without the colon
 */
export function openRule(scheme) {
  if (scheme === 'https') return 'open';
  if (scheme === 'http') return 'open_unencrypted';
  if (ACTION_SCHEMES.includes(scheme)) return 'confirm_action';
  if (NEVER_OPENED.includes(scheme)) return 'never';
  return 'confirm_scheme';
}

const PUNY_BASE = 36;
const PUNY_TMIN = 1;
const PUNY_TMAX = 26;
const PUNY_SKEW = 38;
const PUNY_DAMP = 700;
const PUNY_INITIAL_BIAS = 72;
const PUNY_INITIAL_N = 128;
const PUNY_MAX = 0x7fffffff;

function punyAdapt(delta, numPoints, firstTime) {
  let d = firstTime ? Math.floor(delta / PUNY_DAMP) : Math.floor(delta / 2);
  d += Math.floor(d / numPoints);
  let k = 0;
  while (d > Math.floor(((PUNY_BASE - PUNY_TMIN) * PUNY_TMAX) / 2)) {
    d = Math.floor(d / (PUNY_BASE - PUNY_TMIN));
    k += PUNY_BASE;
  }
  return k + Math.floor(((PUNY_BASE - PUNY_TMIN + 1) * d) / (d + PUNY_SKEW));
}

function punyDigit(code) {
  if (code >= 0x30 && code <= 0x39) return code - 22; // '0'..'9' are 26..35
  if (code >= 0x41 && code <= 0x5a) return code - 0x41; // 'A'..'Z' are 0..25
  if (code >= 0x61 && code <= 0x7a) return code - 0x61; // 'a'..'z' are 0..25
  return -1;
}

/**
 * Decodes a Punycode string (RFC 3492, the part of an A-label after `xn--`). `null` when the
 * input is not valid Punycode.
 *
 * @param {string} input
 * @returns {string | null}
 */
export function punycodeDecode(input) {
  const output = [];
  const basicEnd = Math.max(input.lastIndexOf('-'), 0);
  for (let j = 0; j < basicEnd; j += 1) {
    const code = input.charCodeAt(j);
    if (code >= 0x80) return null;
    output.push(code);
  }
  let n = PUNY_INITIAL_N;
  let bias = PUNY_INITIAL_BIAS;
  let i = 0;
  let index = basicEnd > 0 ? basicEnd + 1 : 0;
  while (index < input.length) {
    const oldI = i;
    let w = 1;
    for (let k = PUNY_BASE; ; k += PUNY_BASE) {
      if (index >= input.length) return null;
      const digit = punyDigit(input.charCodeAt(index));
      index += 1;
      if (digit < 0 || digit > Math.floor((PUNY_MAX - i) / w)) return null;
      i += digit * w;
      const t = k <= bias ? PUNY_TMIN : k >= bias + PUNY_TMAX ? PUNY_TMAX : k - bias;
      if (digit < t) break;
      if (w > Math.floor(PUNY_MAX / (PUNY_BASE - t))) return null;
      w *= PUNY_BASE - t;
    }
    const length = output.length + 1;
    bias = punyAdapt(i - oldI, length, oldI === 0);
    if (Math.floor(i / length) > PUNY_MAX - n) return null;
    n += Math.floor(i / length);
    i %= length;
    if (n > 0x10ffff || (n >= 0xd800 && n <= 0xdfff)) return null;
    output.splice(i, 0, n);
    i += 1;
  }
  return String.fromCodePoint(...output);
}

/** Whether a host name has a label that is an A-label (starts with `xn--`). */
export function isIdnHost(hostname) {
  return hostname.split('.').some((label) => label.toLowerCase().startsWith('xn--'));
}

/**
 * The Unicode form of a host name: every `xn--` label decoded from Punycode. The host name
 * itself when a label does not decode.
 */
export function hostToUnicode(hostname) {
  const labels = hostname.split('.');
  const decoded = [];
  for (const label of labels) {
    if (label.toLowerCase().startsWith('xn--')) {
      const unicode = punycodeDecode(label.slice(4));
      if (unicode === null || unicode === '') return hostname;
      decoded.push(unicode);
    } else {
      decoded.push(label);
    }
  }
  return decoded.join('.');
}

/**
 * What the reader shows for a URL record (3.4.3, URL rules 1 to 5).
 *
 * - `text`: the complete value with every hidden character escaped, line feed and tab
 *   included (rule 2).
 * - `parsed`: whether the WHATWG URL parser accepted it (rule 1). When false, the value is
 *   shown as text with a notice and never offered to open.
 * - `host`: the host in Unicode, escaped, shown on its own line; `null` when the URL has none
 *   (rule 3).
 * - `aLabel`: the A-label form of the host when it is an internationalised domain name,
 *   else `null` (rule 3).
 * - `hasUserinfo`: the URL has a user name or password before `@` (rule 4).
 * - `scheme`, `rule` ({@link openRule}) and `href`, the parsed URL that is opened (rule 5).
 *
 * @param {string} value the URL record's value
 */
export function presentUrl(value) {
  const text = escapeHidden(value);
  let url;
  try {
    url = new URL(value);
  } catch {
    return { parsed: false, text, host: null, aLabel: null, hasUserinfo: false, scheme: '', rule: 'never', href: '' };
  }
  const scheme = url.protocol.replace(/:$/, '').toLowerCase();
  const hostname = url.hostname;
  return {
    parsed: true,
    text,
    host: hostname === '' ? null : escapeHidden(hostToUnicode(hostname)),
    aLabel: hostname !== '' && isIdnHost(hostname) ? escapeHidden(hostname) : null,
    hasUserinfo: url.username !== '' || url.password !== '',
    scheme,
    rule: openRule(scheme),
    href: url.href,
  };
}

// ---------------------------------------------------------------------------------------------
// File names (type 5)

const DEVICE_NAMES = Object.freeze(['CON', 'PRN', 'AUX', 'NUL']);
const REPLACED_IN_NAMES = ':*?"<>|';

/** The longest file name in bytes of UTF-8 (3.4.3, file-name rule 6). */
export const MAX_FILE_NAME_BYTES = 255;

function asciiUpper(text) {
  return text.replace(/[a-z]/g, (c) => c.toUpperCase());
}

/** Whether `stem`, the part of a name before its first `.`, is a Windows device name. */
export function isDeviceName(stem) {
  const upper = asciiUpper(stem);
  return DEVICE_NAMES.includes(upper) || /^(COM|LPT)[0-9]$/.test(upper);
}

function utf8Length(cp) {
  if (cp < 0x80) return 1;
  if (cp < 0x800) return 2;
  if (cp < 0x10000) return 3;
  return 4;
}

/**
 * The name the reader offers when saving a file, by the file-name rules of 3.4.3, in order:
 *
 * 1. keep the part after the last `/` or `\`;
 * 2. remove the hidden characters, every control character included;
 * 3. replace each of `:` `*` `?` `"` `<` `>` `|` by `_`;
 * 4. remove leading dots, then trailing dots and spaces;
 * 5. put `_` before a name whose part before the first `.` is a Windows device name;
 * 6. cut the name to at most 255 bytes of UTF-8, at a character boundary.
 *
 * An empty result means the reader chooses a name of its own ({@link fallbackFileName}).
 *
 * @param {string} value
 */
export function safeFileName(value) {
  const last = value.split(/[/\\]/).pop() ?? '';
  const kept = [];
  for (const ch of last) {
    const cp = ch.codePointAt(0) ?? 0;
    if (isHiddenCodePoint(cp, false)) continue;
    kept.push(REPLACED_IN_NAMES.includes(ch) ? '_' : ch);
  }
  let name = kept.join('').replace(/^\.+/, '').replace(/[. ]+$/, '');
  if (isDeviceName(name.split('.')[0] ?? '')) name = `_${name}`;
  let bytes = 0;
  let end = 0;
  for (const ch of name) {
    bytes += utf8Length(ch.codePointAt(0) ?? 0);
    if (bytes > MAX_FILE_NAME_BYTES) break;
    end += ch.length;
  }
  return name.slice(0, end);
}

/**
 * The final extension of a file name, shown apart from the name before saving (3.4.3), for
 * example `.exe`; `''` when there is none.
 */
export function fileExtension(name) {
  const dot = name.lastIndexOf('.');
  return dot > 0 && dot < name.length - 1 ? name.slice(dot) : '';
}

/**
 * The name the page chooses when a record has no usable file name: `nmtcode-<n>` with an
 * extension by what the record is.
 *
 * @param {string} presentAs `text`, `url`, `file`, `psbt`, `bytes` or `unknown`
 * @param {number} contentType the content type ID
 * @param {number} n the record's number, from 1
 */
export function fallbackFileName(presentAs, contentType, n) {
  let extension = '.bin';
  if (presentAs === 'psbt') extension = '.psbt';
  else if (presentAs === 'text' || presentAs === 'url') extension = contentType === 3 ? '.json' : '.txt';
  return `nmtcode-${n}${extension}`;
}

// ---------------------------------------------------------------------------------------------
// Bytes

/**
 * The first `max` bytes as hexadecimal pairs separated by spaces, 16 to a line.
 *
 * @param {Uint8Array} bytes
 * @returns {{text: string, truncated: boolean, shown: number, total: number}}
 */
export function hexLines(bytes, max) {
  const shown = Math.min(bytes.length, max);
  const lines = [];
  for (let start = 0; start < shown; start += 16) {
    const line = [];
    for (let i = start; i < Math.min(start + 16, shown); i += 1) {
      line.push(bytes[i].toString(16).toUpperCase().padStart(2, '0'));
    }
    lines.push(line.join(' '));
  }
  return { text: lines.join('\n'), truncated: shown < bytes.length, shown, total: bytes.length };
}

// ---------------------------------------------------------------------------------------------
// Strings

/**
 * `template` with each `{name}` replaced by `values[name]`. Unknown names stay as they are.
 *
 * @param {string} template
 * @param {Record<string, string | number>} [values]
 */
export function fill(template, values = {}) {
  return template.replace(/\{([a-z_]+)\}/g, (whole, name) =>
    Object.prototype.hasOwnProperty.call(values, name) ? String(values[name]) : whole,
  );
}

/** `ko` when the browser's first preferred language is Korean, else `en`. */
export function pickLanguage(languages) {
  const first = (languages ?? []).find((tag) => typeof tag === 'string' && tag !== '') ?? '';
  return first.toLowerCase().split('-')[0] === 'ko' ? 'ko' : 'en';
}
