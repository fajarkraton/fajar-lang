# Safety Certification Roadmap

## Vision

Fajar Lang's compiler-enforced context isolation (`@kernel`/`@device`/`@safe`) and effect system provide a strong foundation for safety certification in regulated industries.

## Target Standards

| Standard | Industry | Status | Notes |
|----------|----------|--------|-------|
| **ISO 26262** | Automotive | Planned | ASIL-D for flight-critical software |
| **DO-178C** | Aerospace | Planned | DAL-A for drone flight controllers |
| **IEC 62304** | Medical | Planned | Class C for AI-assisted diagnostics |
| **IEC 61508** | Industrial | Planned | SIL 4 for safety-critical control |

## Fajar Lang Safety Features

### Already Implemented

1. **Context isolation** — compiler prevents cross-domain access
2. **Effect tracking** — side effects declared and verified
3. **Ownership** — no use-after-free, no double-free
4. **Borrow checking** — prevents dangling references
5. **Null safety** — Option<T> instead of null pointers
6. **Integer overflow** — checked arithmetic (no silent wraparound)
7. **Bounds checking** — array access validated
8. **Linear types** — resources must be consumed

### Needed for Certification

1. **Formal proof** — mathematical proof that well-typed programs are safe
2. **MC/DC coverage** — modified condition/decision coverage tooling
3. **Traceability** — requirements → code → tests mapping tool
4. **Tool qualification** — TQL-5 qualification of the Fajar Lang compiler
5. **Deterministic compilation** — reproducible builds (infrastructure exists)
6. **Static analysis** — MISRA-like rule checker for .fj code

## Roadmap

### Phase 1: Foundation (3-6 months)
- Formal semantics document for context annotation system
- MC/DC coverage measurement tool
- Traceability matrix template for Fajar Lang projects

### Phase 2: Tool Qualification (6-12 months)
- Compiler test suite coverage analysis
- Formal verification of context-effect lattice
- Independent review of compiler correctness

### Phase 3: Certification Support (12-18 months)
- Template safety case for Fajar Lang projects
- Pre-qualified compiler for DO-178C DAL-C
- Partnership with certification authority
