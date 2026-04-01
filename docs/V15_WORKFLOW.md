# V15 "Delivery" — Sprint Workflow

> TDD workflow for every V15 sprint. No exceptions.
> **Key difference from V14:** Every task is verified by `fj run`, not just `cargo test`.

---

## Per-Task Workflow

```
1. READ    → Read task from V15_TASKS.md
           → Read V15_SKILLS.md for implementation pattern
           → Understand the SPECIFIC gap being closed

2. TEST    → Write a .fj test file FIRST that exercises the feature
           → Run `fj run test_file.fj` — must FAIL (proves gap exists)
           → Write Rust #[test] if applicable

3. IMPL    → Write MINIMAL code to make the test pass
           → Follow V15_SKILLS.md patterns
           → No extra features beyond what the task requires

4. VERIFY  → Run `fj run test_file.fj` — must PASS
           → Run `cargo test --lib` — all tests pass
           → Run `cargo clippy -- -D warnings` — 0 warnings
           → No .unwrap() in src/

5. MARK    → Mark task [x] in V15_TASKS.md ONLY if `fj run` works
           → If only cargo test passes but fj run doesn't → mark [f]
           → Commit: `feat(v15): B1.1 — fix effect multi-step continuation`
```

---

## Per-Sprint Workflow

```
1. START   → Read all 10 tasks in the sprint
           → Create test .fj files for each task
           → Run all 10 — confirm they all fail (baseline)

2. IMPLEMENT → Work through tasks 1-10 sequentially
             → Each task: test → implement → verify → mark

3. VALIDATE → Run ALL 10 test .fj files — must all pass
           → Run full test suite: cargo test --lib
           → Run clippy: cargo clippy -- -D warnings
           → No regressions from previous sprints

4. COMMIT  → Single commit per sprint (or per task if complex)
           → Format: `feat(v15): complete sprint B1 — effect system fixes`
           → Push to origin

5. UPDATE  → Update V15_TASKS.md summary table
           → Move to next sprint
```

---

## Verification Hierarchy

```
Level 1: cargo test --lib          → Internal API works (MINIMUM)
Level 2: fj run test.fj            → User-facing feature works (REQUIRED for [x])
Level 3: fj check / fj verify      → Analyzer catches errors (for error-path tests)
Level 4: Real-world .fj program    → Feature works in realistic context (for I-sprints)
```

**Rule:** Level 1 alone = [f]. Must reach Level 2+ for [x].

---

## Commit Convention

```
feat(v15): B1.1 — fix effect multi-step continuation
fix(v15): B2.4 — wire Dense.forward() method dispatch  
test(v15): I1.10 — MNIST training tutorial document
docs(v15): D1.1 — effect system tutorial
chore(v15): D3.1 — bump to v12.1.0
```

---

## Sprint Dependencies

```
B1 (Effects) ──→ B2 (ML) ──→ B3 (Toolchain) ──→ I1 (MNIST)
                                                ──→ I2 (FFI)
                                                ──→ I3 (CLI Tools)
                                                    ↓
                                              P1 (Fuzz) ──→ P2 (Bench) ──→ P3 (Security)
                                                                              ↓
                                                                        D1 (Tutorials) ──→ D2 (Gap) ──→ D3 (Release)
```

---

## Quality Gate Checklist (Copy per Sprint)

```
Sprint: ____

Pre-Sprint:
- [ ] Read all 10 tasks
- [ ] Create 10 test .fj files
- [ ] All 10 tests fail (baseline confirmed)

Implementation:
- [ ] Task 1: test passes with `fj run`
- [ ] Task 2: test passes with `fj run`
- [ ] Task 3: test passes with `fj run`
- [ ] Task 4: test passes with `fj run`
- [ ] Task 5: test passes with `fj run`
- [ ] Task 6: test passes with `fj run`
- [ ] Task 7: test passes with `fj run`
- [ ] Task 8: test passes with `fj run`
- [ ] Task 9: test passes with `fj run`
- [ ] Task 10: test passes with `fj run`

Post-Sprint:
- [ ] cargo test --lib — ALL pass (no regressions)
- [ ] cargo clippy -- -D warnings — 0 warnings
- [ ] cargo fmt -- --check — formatted
- [ ] No .unwrap() added to src/
- [ ] V15_TASKS.md updated
- [ ] Committed and pushed
```

---

*V15 Workflow — Version 1.0 | 2026-04-01*
