# Aspirational Examples

> **Status: forward-looking. These programs reference syntax/features
> that are NOT YET IMPLEMENTED in the current Fajar Lang compiler.**

This directory holds examples that describe planned syntax for future
features. They do not currently parse or run via `fj run`. They are
preserved here as design references and to communicate intent — not as
working samples.

Per `docs/1/STRATEGIC_COMPASS.md` §5.2 ("Pangkas Klaim README"):
> *"Setiap klaim harus diverifikasi dengan test atau benchmark yang
> reproducible. Jika tidak bisa diverifikasi, pangkas."*

Examples here are **honest-deferred** — they document what we want
the language to look like, separate from the working `examples/`
proper which the [examples sweep audit](../../docs/EXAMPLES_SWEEP_2026_05_07.md)
shows pass at 95.1% on `fj run`.

## Files in this directory

| File | What it demonstrates | What's missing |
|---|---|---|
| `distributed_mnist.fj` | `@distributed` annotation for cluster ML training | Distributed runtime (Raft) is in `src/distributed/` but the `@distributed` lexer keyword is not wired. Per kompas §5.1 also flagged for "side library" relocation since not relevant to embedded niche. |
| `ffi_numpy.fj` | `@ffi("python")` syntax for Python interop | FFI v2 supports Python via pyo3 (`--features python-ffi`) but this annotation form `@ffi("python")` is not parsed. Existing FFI uses `extern "python" fn ...` syntax. |
| `ffi_opencv.fj` | `@ffi("c++")` syntax for C++ interop | Same as ffi_numpy — `@ffi("c++")` annotation form not parsed. Use `extern "C++" fn ...` syntax. |
| `wasi_http_server.fj` | Async HTTP router with closures-as-arg: `router.get("/", fn(req) -> Response { ... })` | ~~Closure-with-capture as call-argument~~ **CLOSED 2026-06-12** (S2.6 — tagged ClosureHandle + `__closure_call_dyn_N` dispatch; the `#[ignore]` is removed and `native_closure_as_arg_with_capture` passes). Remaining for this example: the async router API itself (WASI P2 extracted to `fajar-wasi-p2` at v36.0.0; HTTP-server surface not on the embedded hot path). |

## Why we keep them

1. **Design intent**: showing what the API should look like guides
   future work.
2. **Honest scope**: separating aspirational from working examples
   means `examples/*.fj` only contains code that runs.
3. **Onboarding**: a new contributor browsing `examples/` shouldn't
   bump into syntax errors and assume the language is broken.

## When to promote a file out of here

When the underlying feature lands AND the example file actually parses
and runs via `fj run` (or via the `tests/selfhost_*.rs` harness for
self-host examples), `git mv` it back to `examples/`. Update this
README.

## Verification

```
$ for f in examples/aspirational/*.fj; do
    timeout 10 ./target/release/fj run "$f" 2>&1 | head -1
  done
```

Each should produce a parser/lexer error (LE001/PE001/PE002) — that's
the expected behavior. If any file in this directory begins to PASS,
the underlying feature is done; promote the file to `examples/`.

---

*Last updated 2026-05-07 per RE_AUDIT_2026_05_07 NEW-4 hygiene
follow-up. See `docs/EXAMPLES_SWEEP_2026_05_07.md` for the full sweep.*
