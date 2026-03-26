// ═══════════════════════════════════════════════════
// Share & Embed — URL encoding, share links, iframe
// ═══════════════════════════════════════════════════

const BASE_URL = 'https://play.fajarlang.dev';

export function encodeForUrl(source) {
  // LZ-string compression if available, otherwise percent-encoding
  try {
    if (typeof LZString !== 'undefined') {
      return LZString.compressToEncodedURIComponent(source);
    }
  } catch (e) { /* fallback */ }

  return encodeURIComponent(source);
}

export function decodeFromUrl(encoded) {
  try {
    if (typeof LZString !== 'undefined') {
      const decoded = LZString.decompressFromEncodedURIComponent(encoded);
      if (decoded) return decoded;
    }
  } catch (e) { /* fallback */ }

  return decodeURIComponent(encoded);
}

export function generateShareUrl(code) {
  const encoded = encodeForUrl(code);
  return `${BASE_URL}/#code=${encoded}`;
}

export function generateEmbed(code, options = {}) {
  const encoded = encodeForUrl(code);
  const theme = options.theme || 'dark';
  const readonly = options.readonly ? '&readonly=true' : '';
  const autorun = options.autorun ? '&autorun=true' : '';
  return `<iframe src="${BASE_URL}/embed?code=${encoded}&theme=${theme}${readonly}${autorun}" width="100%" height="400" frameborder="0" allow="clipboard-write"></iframe>`;
}

export function shortUrlId(code) {
  // FNV-1a hash
  let hash = 0xcbf29ce484222325n;
  for (let i = 0; i < code.length; i++) {
    hash ^= BigInt(code.charCodeAt(i));
    hash = (hash * 0x100000001b3n) & 0xFFFFFFFFFFFFFFFFn;
  }
  return hash.toString(16).slice(0, 8);
}

export function generateQRData(code) {
  return generateShareUrl(code);
}
