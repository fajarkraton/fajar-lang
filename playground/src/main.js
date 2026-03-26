// ═══════════════════════════════════════════════════
// Fajar Lang Playground — Main Entry Point
// Monaco Editor + Wasm Interpreter + Share/Embed
// ═══════════════════════════════════════════════════

import { setupEditor, getCode, setCode, onCursorChange } from './editor.js';
import { initWorker, runCode, tokenize, parseAST, checkCode } from './worker.js';
import { examples, loadExample } from './examples.js';
import { encodeForUrl, decodeFromUrl, generateShareUrl, generateEmbed } from './share.js';

// ── State ──
let isDark = true;
let activeTab = 'stdout';

// ── Init ──
document.addEventListener('DOMContentLoaded', async () => {
  // 1. Setup Monaco editor
  const editor = await setupEditor('editor-container', isDark);
  updateLoadingProgress('Initializing editor');

  // 2. Load code from URL hash or localStorage
  const urlCode = loadCodeFromUrl();
  if (urlCode) {
    setCode(editor, urlCode);
  } else {
    const saved = localStorage.getItem('fj-playground-code');
    if (saved) {
      setCode(editor, saved);
    } else {
      setCode(editor, DEFAULT_CODE);
    }
  }

  // 3. Init Wasm worker
  updateLoadingProgress('Loading Wasm runtime');
  try {
    await initWorker();
    document.getElementById('wasm-status').textContent = 'Wasm ready';
  } catch (e) {
    document.getElementById('wasm-status').textContent = 'Wasm: fallback mode';
    console.warn('Wasm init failed, using fallback:', e);
  }

  // 4. Populate examples
  populateExamples();

  // 5. Setup event listeners
  setupEventListeners(editor);

  // 6. Cursor position tracking
  onCursorChange(editor, (ln, col) => {
    document.getElementById('cursor-pos').textContent = `Ln ${ln}, Col ${col}`;
  });

  // 7. Auto-save
  let saveTimeout;
  editor.onDidChangeModelContent(() => {
    clearTimeout(saveTimeout);
    saveTimeout = setTimeout(() => {
      localStorage.setItem('fj-playground-code', getCode(editor));
    }, 1000);
  });

  // 8. Hide loading overlay
  document.getElementById('loading-overlay').classList.add('hidden');
});

// ── Default code ──
const DEFAULT_CODE = `// Welcome to Fajar Lang Playground!
// Press Ctrl+Enter to run

fn fibonacci(n: i32) -> i32 {
    if n <= 1 { return n }
    fibonacci(n - 1) + fibonacci(n - 2)
}

fn main() {
    println("Fajar Lang v5.5.0 Playground")
    println("============================")

    for i in 0..10 {
        println(f"fib({i}) = {fibonacci(i)}")
    }

    // Pipeline operator
    let result = 5 |> double |> add_one
    println(f"5 |> double |> add_one = {result}")
}

fn double(x: i32) -> i32 { x * 2 }
fn add_one(x: i32) -> i32 { x + 1 }
`;

// ── URL code loading ──
function loadCodeFromUrl() {
  const hash = window.location.hash;
  if (hash.startsWith('#code=')) {
    try {
      return decodeFromUrl(hash.slice(6));
    } catch (e) {
      console.warn('Failed to decode URL code:', e);
    }
  }
  return null;
}

function updateLoadingProgress(text) {
  const el = document.getElementById('loading-progress');
  if (el) el.textContent = text;
}

// ── Examples ──
function populateExamples() {
  const select = document.getElementById('example-select');
  examples.forEach(ex => {
    const opt = document.createElement('option');
    opt.value = ex.slug;
    opt.textContent = `${ex.difficulty === 'advanced' ? '★★★' : ex.difficulty === 'intermediate' ? '★★' : '★'} ${ex.title}`;
    select.appendChild(opt);
  });
}

// ── Event listeners ──
function setupEventListeners(editor) {
  // Run button
  document.getElementById('btn-run').addEventListener('click', () => executeCode(editor));

  // Format button
  document.getElementById('btn-format').addEventListener('click', () => {
    editor.getAction('editor.action.formatDocument')?.run();
  });

  // Clear button
  document.getElementById('btn-clear').addEventListener('click', clearOutput);

  // Theme toggle
  document.getElementById('btn-theme').addEventListener('click', () => toggleTheme(editor));

  // Share button
  document.getElementById('btn-share').addEventListener('click', () => openShareModal(editor));
  document.getElementById('share-close').addEventListener('click', closeShareModal);
  document.getElementById('btn-copy-url').addEventListener('click', () => copyToClipboard('share-url'));
  document.getElementById('btn-copy-embed').addEventListener('click', () => copyToClipboard('share-embed'));

  // Example selector
  document.getElementById('example-select').addEventListener('change', (e) => {
    if (e.target.value) {
      const ex = loadExample(e.target.value);
      if (ex) setCode(editor, ex.code);
      e.target.value = '';
    }
  });

  // Output tabs
  document.querySelectorAll('.tab').forEach(tab => {
    tab.addEventListener('click', () => switchTab(tab.dataset.tab));
  });

  // Keyboard shortcuts
  document.addEventListener('keydown', (e) => {
    if ((e.ctrlKey || e.metaKey) && e.key === 'Enter') {
      e.preventDefault();
      executeCode(editor);
    }
    if ((e.ctrlKey || e.metaKey) && e.key === 'l') {
      e.preventDefault();
      clearOutput();
    }
    if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key === 'S') {
      e.preventDefault();
      openShareModal(editor);
    }
  });

  // Resize handle
  setupResize();

  // Close modal on backdrop click
  document.getElementById('share-modal').addEventListener('click', (e) => {
    if (e.target.id === 'share-modal') closeShareModal();
  });
}

// ── Execute ──
async function executeCode(editor) {
  const code = getCode(editor);
  const statusEl = document.getElementById('status-text');
  const timeEl = document.getElementById('exec-time');

  statusEl.textContent = 'Running...';
  document.getElementById('btn-run').disabled = true;

  try {
    const start = performance.now();
    const result = await runCode(code);
    const elapsed = (performance.now() - start).toFixed(1);

    timeEl.textContent = `${elapsed}ms`;

    // Populate output tabs
    document.getElementById('output-stdout').textContent = result.stdout || '(no output)';
    document.getElementById('output-result').textContent = result.result || 'null';

    if (result.errors && result.errors.length > 0) {
      document.getElementById('output-errors').textContent = result.errors.join('\n\n');
      document.getElementById('output-errors').classList.add('output-error');
      document.querySelector('[data-tab="errors"]').classList.add('has-error');
      switchTab('errors');
      statusEl.textContent = `Error (${elapsed}ms)`;
    } else {
      document.getElementById('output-errors').textContent = '(no errors)';
      document.getElementById('output-errors').classList.remove('output-error');
      document.querySelector('[data-tab="errors"]').classList.remove('has-error');
      switchTab('stdout');
      statusEl.textContent = `Done (${elapsed}ms)`;
    }

    // AST + tokens (async, non-blocking)
    parseAST(code).then(ast => {
      document.getElementById('output-ast').textContent = ast || '(parse error)';
    });
    tokenize(code).then(tokens => {
      document.getElementById('output-tokens').textContent = tokens || '(lex error)';
    });

  } catch (e) {
    document.getElementById('output-errors').textContent = `Runtime error: ${e.message}`;
    switchTab('errors');
    statusEl.textContent = 'Error';
  } finally {
    document.getElementById('btn-run').disabled = false;
  }
}

// ── Tabs ──
function switchTab(tabName) {
  activeTab = tabName;
  document.querySelectorAll('.tab').forEach(t => t.classList.toggle('active', t.dataset.tab === tabName));
  document.querySelectorAll('.output-pane').forEach(p => p.classList.toggle('active', p.id === `output-${tabName}`));
}

function clearOutput() {
  document.querySelectorAll('.output-pane').forEach(p => p.textContent = '');
  document.getElementById('exec-time').textContent = '';
  document.getElementById('status-text').textContent = 'Ready';
}

// ── Theme ──
function toggleTheme(editor) {
  isDark = !isDark;
  document.documentElement.setAttribute('data-theme', isDark ? 'dark' : 'light');
  document.getElementById('btn-theme').textContent = isDark ? 'Dark' : 'Light';
  monaco.editor.setTheme(isDark ? 'fajar-dark' : 'fajar-light');
  localStorage.setItem('fj-playground-theme', isDark ? 'dark' : 'light');
}

// ── Share modal ──
function openShareModal(editor) {
  const code = getCode(editor);
  const url = generateShareUrl(code);
  const embed = generateEmbed(code);

  document.getElementById('share-url').value = url;
  document.getElementById('share-embed').value = embed;
  document.getElementById('share-modal').classList.remove('hidden');

  // Update browser URL without reload
  history.replaceState(null, '', `#code=${encodeForUrl(code)}`);
}

function closeShareModal() {
  document.getElementById('share-modal').classList.add('hidden');
}

function copyToClipboard(elementId) {
  const el = document.getElementById(elementId);
  el.select();
  navigator.clipboard.writeText(el.value).then(() => {
    const btn = el.nextElementSibling;
    const orig = btn.textContent;
    btn.textContent = 'Copied!';
    setTimeout(() => btn.textContent = orig, 1500);
  });
}

// ── Resize ──
function setupResize() {
  const handle = document.getElementById('resize-handle');
  const editorPanel = document.getElementById('editor-panel');
  const outputPanel = document.getElementById('output-panel');
  let isResizing = false;

  handle.addEventListener('mousedown', (e) => {
    isResizing = true;
    handle.classList.add('active');
    document.body.style.cursor = 'col-resize';
    document.body.style.userSelect = 'none';
  });

  document.addEventListener('mousemove', (e) => {
    if (!isResizing) return;
    const mainRect = document.getElementById('main').getBoundingClientRect();
    const fraction = (e.clientX - mainRect.left) / mainRect.width;
    const clamped = Math.max(0.2, Math.min(0.8, fraction));
    editorPanel.style.flex = `${clamped}`;
    outputPanel.style.flex = `${1 - clamped}`;
  });

  document.addEventListener('mouseup', () => {
    if (isResizing) {
      isResizing = false;
      handle.classList.remove('active');
      document.body.style.cursor = '';
      document.body.style.userSelect = '';
    }
  });
}
