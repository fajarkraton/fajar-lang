// ═══════════════════════════════════════════════════
// Wasm Worker Bridge — execute Fajar Lang in Web Worker
// Falls back to demo mode if Wasm unavailable
// ═══════════════════════════════════════════════════

let worker = null;
let wasmReady = false;
let requestId = 0;
let pendingRequests = new Map();

// ── Init Worker ──
export async function initWorker() {
  return new Promise((resolve, reject) => {
    try {
      worker = new Worker(new URL('./executor.js', import.meta.url), { type: 'module' });

      worker.onmessage = (e) => {
        const { id, type, data } = e.data;

        if (type === 'ready') {
          wasmReady = true;
          resolve();
          return;
        }

        if (type === 'init-error') {
          console.warn('Worker Wasm init failed, using fallback');
          wasmReady = false;
          resolve(); // resolve anyway, we'll use fallback
          return;
        }

        const pending = pendingRequests.get(id);
        if (pending) {
          pendingRequests.delete(id);
          if (type === 'error') {
            pending.reject(new Error(data.message));
          } else {
            pending.resolve(data);
          }
        }
      };

      worker.onerror = (e) => {
        console.warn('Worker error:', e);
        wasmReady = false;
        resolve(); // fallback mode
      };

      // Timeout for init
      setTimeout(() => {
        if (!wasmReady) {
          console.warn('Worker init timeout, using fallback');
          resolve();
        }
      }, 10000);
    } catch (e) {
      console.warn('Worker creation failed:', e);
      reject(e);
    }
  });
}

// ── Send request to worker ──
function sendRequest(type, payload) {
  const id = ++requestId;
  return new Promise((resolve, reject) => {
    // Timeout per request
    const timeout = setTimeout(() => {
      pendingRequests.delete(id);
      reject(new Error('Execution timeout (5s)'));
      // Kill and restart worker
      if (worker) {
        worker.terminate();
        initWorker().catch(() => {});
      }
    }, 5000);

    pendingRequests.set(id, {
      resolve: (data) => { clearTimeout(timeout); resolve(data); },
      reject: (err) => { clearTimeout(timeout); reject(err); },
    });

    if (worker && wasmReady) {
      worker.postMessage({ id, type, ...payload });
    } else {
      // Fallback: simulate execution
      clearTimeout(timeout);
      pendingRequests.delete(id);
      resolve(fallbackExecute(type, payload));
    }
  });
}

// ── Public API ──
export async function runCode(source) {
  return sendRequest('run', { source });
}

export async function tokenize(source) {
  const result = await sendRequest('tokenize', { source });
  return result.tokens || JSON.stringify(result, null, 2);
}

export async function parseAST(source) {
  const result = await sendRequest('parse', { source });
  return result.ast || JSON.stringify(result, null, 2);
}

export async function checkCode(source) {
  return sendRequest('check', { source });
}

// ── Fallback execution (demo mode without Wasm) ──
function fallbackExecute(type, payload) {
  const { source } = payload;

  if (type === 'tokenize') {
    return { tokens: fallbackTokenize(source) };
  }

  if (type === 'parse') {
    return { ast: '(AST not available in fallback mode)' };
  }

  if (type === 'check') {
    return { ok: true, errors: [] };
  }

  // type === 'run'
  return fallbackRun(source);
}

function fallbackRun(source) {
  const lines = source.split('\n');
  let output = [];
  let hasError = false;
  let errors = [];

  // Simple pattern matching for println/print calls
  for (const line of lines) {
    const trimmed = line.trim();

    // Match println("...")
    const printMatch = trimmed.match(/println\((?:f)?"([^"]*)"\)/);
    if (printMatch) {
      let text = printMatch[1];
      // Simple f-string interpolation (just replace {expr} with ?)
      text = text.replace(/\{[^}]+\}/g, '?');
      output.push(text);
    }

    // Match println(f"...{var}...")
    const fprintMatch = trimmed.match(/println\(f"([^"]*)"\)/);
    if (fprintMatch) {
      let text = fprintMatch[1];
      text = text.replace(/\{[^}]+\}/g, '?');
      if (!output.includes(text)) output.push(text);
    }
  }

  if (output.length === 0) {
    output.push('(fallback mode: Wasm runtime not loaded)');
    output.push('(install wasm-pack and run: npm run wasm:build)');
    output.push('');
    output.push('Detected code structure:');
    const fnCount = (source.match(/\bfn\b/g) || []).length;
    const structCount = (source.match(/\bstruct\b/g) || []).length;
    const enumCount = (source.match(/\benum\b/g) || []).length;
    output.push(`  Functions: ${fnCount}`);
    output.push(`  Structs: ${structCount}`);
    output.push(`  Enums: ${enumCount}`);
    output.push(`  Lines: ${lines.length}`);
  }

  return {
    success: !hasError,
    result: null,
    stdout: output.join('\n'),
    errors,
    elapsed_ms: 0,
    instructions: 0,
    peak_memory_bytes: 0,
  };
}

function fallbackTokenize(source) {
  const tokens = [];
  const keywords = new Set(['fn', 'let', 'mut', 'const', 'struct', 'enum', 'impl', 'trait',
    'if', 'else', 'match', 'while', 'for', 'in', 'return', 'break', 'continue', 'loop']);

  const words = source.match(/[a-zA-Z_]\w*|"[^"]*"|\d+\.?\d*|\/\/[^\n]*|[|><=!+\-*/&^%{}()\[\];,.]/g) || [];

  for (const word of words) {
    let kind = 'Ident';
    if (keywords.has(word)) kind = 'Keyword';
    else if (/^\d/.test(word)) kind = 'Number';
    else if (word.startsWith('"')) kind = 'String';
    else if (word.startsWith('//')) kind = 'Comment';
    else if (/^[|><=!+\-*/&^%]/.test(word)) kind = 'Operator';
    else if (/^[{}()\[\];,.]$/.test(word)) kind = 'Punct';
    tokens.push(`${kind.padEnd(10)} ${word}`);
  }

  return tokens.join('\n');
}
