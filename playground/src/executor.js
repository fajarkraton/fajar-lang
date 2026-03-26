// ═══════════════════════════════════════════════════
// Web Worker — Wasm Executor
// Runs in dedicated thread, communicates via postMessage
// ═══════════════════════════════════════════════════

let wasm = null;

// ── Init Wasm ──
async function initWasm() {
  try {
    const module = await import('../pkg/fajar_lang.js');
    await module.default();
    wasm = module;
    self.postMessage({ type: 'ready' });
  } catch (e) {
    console.warn('Wasm init failed:', e.message);
    self.postMessage({ type: 'init-error', data: { message: e.message } });
  }
}

// ── Handle messages ──
self.onmessage = async (e) => {
  const { id, type, source } = e.data;

  try {
    let result;

    switch (type) {
      case 'run':
        if (wasm && wasm.eval_source) {
          const raw = wasm.eval_source(source);
          result = typeof raw === 'string' ? JSON.parse(raw) : raw;
        } else {
          result = {
            success: false,
            stdout: '',
            errors: ['Wasm runtime not available'],
            result: null,
            elapsed_ms: 0,
          };
        }
        break;

      case 'tokenize':
        if (wasm && wasm.tokenize) {
          const raw = wasm.tokenize(source);
          result = { tokens: typeof raw === 'string' ? raw : JSON.stringify(raw, null, 2) };
        } else {
          result = { tokens: '(Wasm not available)' };
        }
        break;

      case 'parse':
        if (wasm && wasm.parse) {
          const raw = wasm.parse(source);
          result = { ast: typeof raw === 'string' ? raw : JSON.stringify(raw, null, 2) };
        } else {
          result = { ast: '(Wasm not available)' };
        }
        break;

      case 'check':
        if (wasm && wasm.check) {
          const raw = wasm.check(source);
          result = typeof raw === 'string' ? JSON.parse(raw) : raw;
        } else {
          result = { ok: true, errors: [] };
        }
        break;

      default:
        result = { error: `Unknown request type: ${type}` };
    }

    self.postMessage({ id, type: 'result', data: result });
  } catch (e) {
    self.postMessage({ id, type: 'error', data: { message: e.message || String(e) } });
  }
};

// ── Start ──
initWasm();
