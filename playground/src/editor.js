// ═══════════════════════════════════════════════════
// Monaco Editor Setup — Fajar Lang syntax + themes
// ═══════════════════════════════════════════════════

let monacoReady = false;

export async function setupEditor(containerId, isDark) {
  // Dynamically import Monaco
  const monaco = await import('monaco-editor');
  window.monaco = monaco;

  // Register Fajar Lang language
  monaco.languages.register({ id: 'fajar' });

  // Monarch tokenizer
  monaco.languages.setMonarchTokensProvider('fajar', {
    keywords: [
      'fn', 'let', 'mut', 'const', 'struct', 'enum', 'impl', 'trait', 'type',
      'if', 'else', 'match', 'while', 'for', 'in', 'return', 'break', 'continue',
      'loop', 'use', 'mod', 'pub', 'extern', 'as', 'where', 'async', 'await',
      'dyn', 'move', 'ref', 'self', 'super', 'crate',
    ],
    typeKeywords: [
      'bool', 'i8', 'i16', 'i32', 'i64', 'i128', 'u8', 'u16', 'u32', 'u64', 'u128',
      'isize', 'usize', 'f32', 'f64', 'str', 'char', 'void', 'never',
      'tensor', 'grad', 'loss', 'layer', 'model',
      'ptr', 'addr', 'page', 'region', 'irq', 'syscall',
    ],
    constants: ['true', 'false', 'null', 'Some', 'None', 'Ok', 'Err', 'PI', 'E'],
    operators: [
      '=', '>', '<', '!', '~', '?', ':', '==', '<=', '>=', '!=',
      '&&', '||', '++', '--', '+', '-', '*', '/', '&', '|', '^', '%',
      '<<', '>>', '>>>', '+=', '-=', '*=', '/=', '&=', '|=', '^=',
      '%=', '<<=', '>>=', '|>', '..', '..=', '=>', '->', '**',
    ],
    tokenizer: {
      root: [
        // Annotations
        [/@(kernel|device|safe|unsafe|ffi|test|should_panic|ignore|entry|interrupt)/, 'annotation'],
        // Identifiers and keywords
        [/[a-zA-Z_]\w*/, {
          cases: {
            '@keywords': 'keyword',
            '@typeKeywords': 'type',
            '@constants': 'constant',
            '@default': 'identifier',
          }
        }],
        // Whitespace
        { include: '@whitespace' },
        // Brackets
        [/[{}()\[\]]/, '@brackets'],
        // Numbers
        [/0[xX][0-9a-fA-F_]+/, 'number.hex'],
        [/0[bB][01_]+/, 'number.binary'],
        [/0[oO][0-7_]+/, 'number.octal'],
        [/\d[\d_]*\.[\d_]*([eE][-+]?\d+)?/, 'number.float'],
        [/\d[\d_]*/, 'number'],
        // Pipeline operator (special)
        [/\|>/, 'operator.pipeline'],
        // Operators
        [/[;,.]/, 'delimiter'],
        [/[=><!~?:&|+\-*/^%]+/, 'operator'],
        // Strings
        [/f"/, { token: 'string.fstring', next: '@fstring' }],
        [/"/, { token: 'string.quote', next: '@string' }],
        // Characters
        [/'[^'\\]'/, 'string.char'],
        [/'\\.'/, 'string.char'],
      ],
      string: [
        [/[^\\"]+/, 'string'],
        [/\\./, 'string.escape'],
        [/"/, { token: 'string.quote', next: '@pop' }],
      ],
      fstring: [
        [/[^\\"{]+/, 'string'],
        [/\{/, { token: 'string.interpolation', next: '@fstringExpr' }],
        [/\\./, 'string.escape'],
        [/"/, { token: 'string.fstring', next: '@pop' }],
      ],
      fstringExpr: [
        [/[^}]+/, 'identifier'],
        [/\}/, { token: 'string.interpolation', next: '@pop' }],
      ],
      whitespace: [
        [/[ \t\r\n]+/, 'white'],
        [/\/\/.*$/, 'comment'],
        [/\/\*/, 'comment', '@comment'],
      ],
      comment: [
        [/[^/*]+/, 'comment'],
        [/\*\//, 'comment', '@pop'],
        [/[/*]/, 'comment'],
      ],
    },
  });

  // Auto-completion
  monaco.languages.registerCompletionItemProvider('fajar', {
    provideCompletionItems: (model, position) => {
      const word = model.getWordUntilPosition(position);
      const range = {
        startLineNumber: position.lineNumber,
        endLineNumber: position.lineNumber,
        startColumn: word.startColumn,
        endColumn: word.endColumn,
      };
      const suggestions = [
        ...['fn', 'let', 'mut', 'const', 'struct', 'enum', 'impl', 'trait', 'if', 'else',
            'match', 'while', 'for', 'in', 'return', 'break', 'continue', 'loop'].map(k => ({
          label: k, kind: monaco.languages.CompletionItemKind.Keyword,
          insertText: k, range,
        })),
        ...['println', 'print', 'len', 'type_of', 'assert', 'assert_eq', 'dbg', 'todo',
            'push', 'pop', 'contains', 'split', 'trim', 'replace', 'parse_int', 'parse_float',
            'sqrt', 'abs', 'sin', 'cos', 'floor', 'ceil', 'round', 'min', 'max',
            'zeros', 'ones', 'randn', 'matmul', 'relu', 'softmax', 'sigmoid', 'backward'].map(f => ({
          label: f, kind: monaco.languages.CompletionItemKind.Function,
          insertText: f, range,
        })),
        ...['i32', 'i64', 'f64', 'str', 'bool', 'char', 'void'].map(t => ({
          label: t, kind: monaco.languages.CompletionItemKind.TypeParameter,
          insertText: t, range,
        })),
      ];
      return { suggestions };
    },
  });

  // Register themes
  registerThemes(monaco);

  // Create editor
  const editor = monaco.editor.create(document.getElementById(containerId), {
    language: 'fajar',
    theme: isDark ? 'fajar-dark' : 'fajar-light',
    fontFamily: "'JetBrains Mono', 'Fira Code', 'Cascadia Code', monospace",
    fontSize: 14,
    lineNumbers: 'on',
    minimap: { enabled: false },
    tabSize: 4,
    automaticLayout: true,
    scrollBeyondLastLine: false,
    padding: { top: 8, bottom: 8 },
    renderLineHighlight: 'line',
    cursorBlinking: 'smooth',
    cursorSmoothCaretAnimation: 'on',
    smoothScrolling: true,
    bracketPairColorization: { enabled: true },
    guides: { bracketPairs: true, indentation: true },
  });

  monacoReady = true;
  return editor;
}

function registerThemes(monaco) {
  monaco.editor.defineTheme('fajar-dark', {
    base: 'vs-dark',
    inherit: true,
    rules: [
      { token: 'keyword', foreground: 'f85149', fontStyle: 'bold' },
      { token: 'type', foreground: '79c0ff' },
      { token: 'annotation', foreground: 'd29922', fontStyle: 'bold' },
      { token: 'constant', foreground: '79c0ff' },
      { token: 'string', foreground: '3fb950' },
      { token: 'string.fstring', foreground: '3fb950' },
      { token: 'string.interpolation', foreground: 'd2a8ff' },
      { token: 'string.escape', foreground: '56d364' },
      { token: 'string.char', foreground: '3fb950' },
      { token: 'number', foreground: 'd2a8ff' },
      { token: 'number.hex', foreground: 'd2a8ff' },
      { token: 'number.float', foreground: 'd2a8ff' },
      { token: 'comment', foreground: '8b949e', fontStyle: 'italic' },
      { token: 'operator.pipeline', foreground: 'ff7b72', fontStyle: 'bold' },
      { token: 'identifier', foreground: 'e6edf3' },
    ],
    colors: {
      'editor.background': '#0d1117',
      'editor.foreground': '#e6edf3',
      'editorLineNumber.foreground': '#484f58',
      'editorLineNumber.activeForeground': '#e6edf3',
      'editor.selectionBackground': '#264f78',
      'editor.lineHighlightBackground': '#161b2280',
    },
  });

  monaco.editor.defineTheme('fajar-light', {
    base: 'vs',
    inherit: true,
    rules: [
      { token: 'keyword', foreground: 'cf222e', fontStyle: 'bold' },
      { token: 'type', foreground: '0550ae' },
      { token: 'annotation', foreground: '953800', fontStyle: 'bold' },
      { token: 'constant', foreground: '0550ae' },
      { token: 'string', foreground: '116329' },
      { token: 'number', foreground: '8250df' },
      { token: 'comment', foreground: '6e7781', fontStyle: 'italic' },
      { token: 'operator.pipeline', foreground: 'cf222e', fontStyle: 'bold' },
    ],
    colors: {
      'editor.background': '#ffffff',
      'editor.foreground': '#1f2328',
    },
  });
}

export function getCode(editor) {
  return editor.getValue();
}

export function setCode(editor, code) {
  editor.setValue(code);
}

export function onCursorChange(editor, callback) {
  editor.onDidChangeCursorPosition((e) => {
    callback(e.position.lineNumber, e.position.column);
  });
}
