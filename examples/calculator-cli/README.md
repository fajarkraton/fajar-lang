# calculator-cli — multi-file Fajar Lang example

A REPL arithmetic calculator demonstrating Fajar Lang's:
- Multi-file project layout (`fj.toml` + `src/main.fj` + `src/lexer.fj`)
- `pub fn` cross-module visibility
- Imperative loops with stack-based shunting-yard evaluation

## Layout

```
calculator-cli/
├── fj.toml          # package manifest
├── README.md        # this file
└── src/
    ├── main.fj      # entry point + evaluator
    └── lexer.fj     # tokenizer (pub fn tokenize)
```

## Build & run

```bash
cd examples/calculator-cli
fj build
fj run
```

Expected output:

```
6 + 7 * 3 - 4 / 2 = 25
```

## What it demonstrates

- **Operator precedence** via Dijkstra-style two-stack evaluator. `*`/`/`
  bind tighter than `+`/`-`.
- **Module imports** via `use lexer::tokenize`.
- **Pure functional style** — no mutable globals; every helper takes
  values and returns values.

## Extending

- Add `(` / `)` paren handling to `evaluate()` (lexer already emits them).
- Read input lines instead of a hardcoded expression — use `read_line()`.
- Track variable assignments: extend tokens with identifiers + `=`.

## Related

- `docs/TUTORIAL.md` Chapter 1 — first calculator program.
- `examples/cli_tools/calc.fj` — single-file version of this calculator.
