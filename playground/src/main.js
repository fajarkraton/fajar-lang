// ===================================================
// Fajar Lang Playground -- Main Entry Point
// Monaco Editor + Wasm Interpreter + Share/Embed
// Enhanced: AST tree viewer, Token viewer, Shortcuts
// ===================================================

import { setupEditor, getCode, setCode, onCursorChange } from './editor.js';
import { initWorker, runCode, tokenize, parseAST, checkCode } from './worker.js';
import { examples, loadExample } from './examples.js';
import { encodeForUrl, decodeFromUrl, generateShareUrl, generateEmbed } from './share.js';

// -- State --
let isDark = true;
let activeTab = 'stdout';

// -- Init --
document.addEventListener('DOMContentLoaded', async () => {
  // 1. Load saved theme
  const savedTheme = localStorage.getItem('fj-playground-theme');
  if (savedTheme === 'light') {
    isDark = false;
    document.documentElement.setAttribute('data-theme', 'light');
    document.getElementById('btn-theme').textContent = 'Light';
  }

  // 2. Setup Monaco editor
  const editor = await setupEditor('editor-container', isDark);
  updateLoadingProgress('Initializing editor');

  // 3. Load code from URL hash or localStorage
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

  // 4. Init Wasm worker
  updateLoadingProgress('Loading Wasm runtime');
  try {
    await initWorker();
    document.getElementById('wasm-status').textContent = 'Wasm ready';
  } catch (e) {
    document.getElementById('wasm-status').textContent = 'Wasm: fallback mode';
    console.warn('Wasm init failed, using fallback:', e);
  }

  // 5. Populate examples
  populateExamples();

  // 6. Setup event listeners
  setupEventListeners(editor);

  // 7. Cursor position tracking
  onCursorChange(editor, (ln, col) => {
    document.getElementById('cursor-pos').textContent = `Ln ${ln}, Col ${col}`;
  });

  // 8. Auto-save
  let saveTimeout;
  editor.onDidChangeModelContent(() => {
    clearTimeout(saveTimeout);
    saveTimeout = setTimeout(() => {
      localStorage.setItem('fj-playground-code', getCode(editor));
    }, 1000);
  });

  // 9. Listen for hash changes (shared URLs)
  window.addEventListener('hashchange', () => {
    const code = loadCodeFromUrl();
    if (code) setCode(editor, code);
  });

  // 10. Hide loading overlay
  document.getElementById('loading-overlay').classList.add('hidden');
});

// -- Default code --
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

// -- URL code loading (base64 + LZ-string) --
function loadCodeFromUrl() {
  const hash = window.location.hash;
  if (hash.startsWith('#code=')) {
    try {
      return decodeFromUrl(hash.slice(6));
    } catch (e) {
      // Try base64 fallback
      try {
        return atob(decodeURIComponent(hash.slice(6)));
      } catch (e2) {
        console.warn('Failed to decode URL code:', e2);
      }
    }
  }
  return null;
}

function updateLoadingProgress(text) {
  const el = document.getElementById('loading-progress');
  if (el) el.textContent = text;
}

// -- Examples --
function populateExamples() {
  const select = document.getElementById('example-select');
  examples.forEach(ex => {
    const opt = document.createElement('option');
    opt.value = ex.slug;
    opt.textContent = `${ex.difficulty === 'advanced' ? '***' : ex.difficulty === 'intermediate' ? '**' : '*'} ${ex.title}`;
    select.appendChild(opt);
  });
}

// -- Event listeners --
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

  // Shortcuts button
  document.getElementById('btn-shortcuts').addEventListener('click', openShortcutsModal);
  document.getElementById('shortcuts-close').addEventListener('click', closeShortcutsModal);

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
    if (e.key === 'F1') {
      e.preventDefault();
      openShortcutsModal();
    }
    if (e.key === 'Escape') {
      closeShareModal();
      closeShortcutsModal();
    }
  });

  // Resize handle
  setupResize();

  // Close modals on backdrop click
  document.getElementById('share-modal').addEventListener('click', (e) => {
    if (e.target.id === 'share-modal') closeShareModal();
  });
  document.getElementById('shortcuts-modal').addEventListener('click', (e) => {
    if (e.target.id === 'shortcuts-modal') closeShortcutsModal();
  });
}

// -- Execute --
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

    // AST viewer (collapsible tree)
    parseAST(code).then(ast => {
      renderASTTree('output-ast', ast);
    });

    // Token viewer (colored table)
    tokenize(code).then(tokens => {
      renderTokenTable('output-tokens', tokens);
    });

  } catch (e) {
    document.getElementById('output-errors').textContent = `Runtime error: ${e.message}`;
    switchTab('errors');
    statusEl.textContent = 'Error';
  } finally {
    document.getElementById('btn-run').disabled = false;
  }
}

// ======================================================
// AST Tree Viewer -- collapsible tree from parsed AST
// ======================================================

function renderASTTree(containerId, astData) {
  const container = document.getElementById(containerId);
  container.innerHTML = '';

  if (!astData || astData === '(parse error)' || astData === '(AST not available in fallback mode)') {
    container.innerHTML = '<div class="ast-empty">' + escapeHtml(astData || 'No AST available. Click Run first.') + '</div>';
    return;
  }

  // Try to parse as JSON
  let ast;
  try {
    ast = typeof astData === 'string' ? JSON.parse(astData) : astData;
  } catch (e) {
    // If not JSON, render as formatted text in a collapsible structure
    ast = parseASTText(astData);
  }

  const tree = buildASTNode(ast, 'Program', 0);
  container.appendChild(tree);
}

function parseASTText(text) {
  // Parse indented text-format AST into a tree structure
  const lines = text.split('\n').filter(l => l.trim());
  if (lines.length === 0) return { type: 'Empty' };

  const root = { type: 'Program', children: [] };
  const stack = [{ node: root, indent: -1 }];

  for (const line of lines) {
    const indent = line.search(/\S/);
    const content = line.trim();

    const newNode = { type: content, children: [] };

    while (stack.length > 1 && stack[stack.length - 1].indent >= indent) {
      stack.pop();
    }

    stack[stack.length - 1].node.children.push(newNode);
    stack.push({ node: newNode, indent });
  }

  return root;
}

function buildASTNode(data, label, depth) {
  const wrapper = document.createElement('div');
  if (depth > 0) wrapper.className = 'ast-node';

  if (data === null || data === undefined) {
    const span = document.createElement('span');
    span.className = 'ast-punct';
    span.textContent = `${label}: null`;
    wrapper.appendChild(span);
    return wrapper;
  }

  if (typeof data === 'string' || typeof data === 'number' || typeof data === 'boolean') {
    const span = document.createElement('span');
    span.innerHTML = `<span class="ast-key">${escapeHtml(label)}</span><span class="ast-punct">: </span><span class="ast-value">${escapeHtml(String(data))}</span>`;
    wrapper.appendChild(span);
    return wrapper;
  }

  if (Array.isArray(data)) {
    if (data.length === 0) {
      const span = document.createElement('span');
      span.innerHTML = `<span class="ast-key">${escapeHtml(label)}</span><span class="ast-punct">: []</span>`;
      wrapper.appendChild(span);
      return wrapper;
    }

    const header = document.createElement('div');
    const toggle = createToggle();
    header.appendChild(toggle);
    const labelSpan = document.createElement('span');
    labelSpan.innerHTML = `<span class="ast-label">${escapeHtml(label)}</span><span class="ast-punct"> [${data.length}]</span>`;
    header.appendChild(labelSpan);
    wrapper.appendChild(header);

    const children = document.createElement('div');
    children.className = 'ast-children';
    data.forEach((item, i) => {
      children.appendChild(buildASTNode(item, `[${i}]`, depth + 1));
    });
    wrapper.appendChild(children);

    toggle.addEventListener('click', () => toggleASTNode(toggle, children));
    return wrapper;
  }

  if (typeof data === 'object') {
    const keys = Object.keys(data);
    // If it has a "type" or "kind" field, use that as the label display
    const typeLabel = data.type || data.kind || data.node_type || '';

    const header = document.createElement('div');
    const toggle = createToggle();
    header.appendChild(toggle);
    const labelSpan = document.createElement('span');
    if (typeLabel && label !== typeLabel) {
      labelSpan.innerHTML = `<span class="ast-label">${escapeHtml(label)}</span><span class="ast-punct">: </span><span class="ast-type">${escapeHtml(typeLabel)}</span>`;
    } else {
      labelSpan.innerHTML = `<span class="ast-label">${escapeHtml(label)}</span>`;
    }
    header.appendChild(labelSpan);
    wrapper.appendChild(header);

    const children = document.createElement('div');
    children.className = 'ast-children';

    // Auto-collapse deep nodes
    if (depth > 3) {
      children.classList.add('collapsed');
      toggle.classList.add('collapsed');
    }

    for (const key of keys) {
      children.appendChild(buildASTNode(data[key], key, depth + 1));
    }
    wrapper.appendChild(children);

    toggle.addEventListener('click', () => toggleASTNode(toggle, children));
    return wrapper;
  }

  wrapper.textContent = `${label}: ${String(data)}`;
  return wrapper;
}

function createToggle() {
  const toggle = document.createElement('span');
  toggle.className = 'ast-toggle';
  toggle.textContent = '\u25BC'; // down arrow
  toggle.title = 'Click to collapse/expand';
  return toggle;
}

function toggleASTNode(toggle, children) {
  const isCollapsed = children.classList.contains('collapsed');
  if (isCollapsed) {
    children.classList.remove('collapsed');
    toggle.classList.remove('collapsed');
  } else {
    children.classList.add('collapsed');
    toggle.classList.add('collapsed');
  }
}

// ======================================================
// Token Viewer -- colored table with token types
// ======================================================

function renderTokenTable(containerId, tokenData) {
  const container = document.getElementById(containerId);
  container.innerHTML = '';

  if (!tokenData || tokenData === '(lex error)' || tokenData === '(Wasm not available)') {
    container.innerHTML = '<div class="token-empty">' + escapeHtml(tokenData || 'No tokens. Click Run first.') + '</div>';
    return;
  }

  // Build table
  const table = document.createElement('table');
  table.className = 'token-table';

  const thead = document.createElement('thead');
  thead.innerHTML = '<tr><th>#</th><th>Kind</th><th>Value</th></tr>';
  table.appendChild(thead);

  const tbody = document.createElement('tbody');

  // Parse token data: could be "Kind       value" lines or JSON
  let tokens;
  try {
    tokens = typeof tokenData === 'string' ? null : tokenData;
    if (typeof tokenData === 'string') {
      try {
        tokens = JSON.parse(tokenData);
      } catch (e) {
        // Parse text format: "Kind       value" per line
        tokens = tokenData.split('\n').filter(l => l.trim()).map((line, i) => {
          const match = line.match(/^(\S+)\s+(.*)$/);
          if (match) {
            return { kind: match[1], value: match[2] };
          }
          return { kind: 'Unknown', value: line.trim() };
        });
      }
    }
  } catch (e) {
    container.innerHTML = '<div class="token-empty">Failed to parse token data</div>';
    return;
  }

  if (Array.isArray(tokens)) {
    tokens.forEach((tok, idx) => {
      const tr = document.createElement('tr');
      const kind = tok.kind || tok.type || 'Unknown';
      const value = tok.value || tok.text || tok.lexeme || '';
      const cssClass = getTokenColorClass(kind);

      tr.innerHTML = `<td class="token-span">${idx + 1}</td><td class="${cssClass}">${escapeHtml(kind)}</td><td>${escapeHtml(value)}</td>`;
      tbody.appendChild(tr);
    });
  }

  table.appendChild(tbody);
  container.appendChild(table);

  // Summary line
  const summary = document.createElement('div');
  summary.className = 'token-empty';
  summary.textContent = `${tokens ? tokens.length : 0} tokens`;
  summary.style.fontStyle = 'normal';
  summary.style.borderTop = '1px solid var(--border)';
  summary.style.marginTop = '8px';
  summary.style.paddingTop = '8px';
  container.appendChild(summary);
}

function getTokenColorClass(kind) {
  const k = kind.toLowerCase();
  if (k === 'keyword' || k === 'kw') return 'tok-keyword';
  if (k === 'ident' || k === 'identifier') return 'tok-ident';
  if (k === 'number' || k === 'int' || k === 'float' || k === 'integer') return 'tok-number';
  if (k === 'string' || k === 'str') return 'tok-string';
  if (k === 'operator' || k === 'op') return 'tok-operator';
  if (k === 'punct' || k === 'delimiter' || k === 'punctuation') return 'tok-punct';
  if (k === 'comment') return 'tok-comment';
  if (k === 'type') return 'tok-type';
  if (k === 'annotation') return 'tok-annotation';
  if (k === 'constant' || k === 'bool' || k === 'true' || k === 'false' || k === 'null') return 'tok-literal';
  return 'tok-ident';
}

// ======================================================
// Utilities
// ======================================================

function escapeHtml(str) {
  const div = document.createElement('div');
  div.textContent = str;
  return div.innerHTML;
}

// -- Tabs --
function switchTab(tabName) {
  activeTab = tabName;
  document.querySelectorAll('.tab').forEach(t => t.classList.toggle('active', t.dataset.tab === tabName));
  document.querySelectorAll('.output-pane').forEach(p => p.classList.toggle('active', p.id === `output-${tabName}`));
}

function clearOutput() {
  document.querySelectorAll('.output-pane').forEach(p => {
    if (p.tagName === 'PRE') {
      p.textContent = '';
    } else {
      p.innerHTML = '';
    }
  });
  document.getElementById('exec-time').textContent = '';
  document.getElementById('status-text').textContent = 'Ready';
}

// -- Theme --
function toggleTheme(editor) {
  isDark = !isDark;
  document.documentElement.setAttribute('data-theme', isDark ? 'dark' : 'light');
  document.getElementById('btn-theme').textContent = isDark ? 'Dark' : 'Light';
  monaco.editor.setTheme(isDark ? 'fajar-dark' : 'fajar-light');
  localStorage.setItem('fj-playground-theme', isDark ? 'dark' : 'light');
}

// -- Share modal --
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

// -- Shortcuts modal --
function openShortcutsModal() {
  document.getElementById('shortcuts-modal').classList.remove('hidden');
}

function closeShortcutsModal() {
  document.getElementById('shortcuts-modal').classList.add('hidden');
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

// -- Resize --
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
