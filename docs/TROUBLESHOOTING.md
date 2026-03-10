# TROUBLESHOOTING

> Panduan Debugging & Solusi Masalah — Fajar Lang

---

## 1. Build & Compilation Issues

| Error | Penyebab | Solusi |
|-------|----------|--------|
| `can't find crate` | Module declaration missing | Tambahkan `pub mod xxx;` di parent `mod.rs` |
| `unresolved import` | Use path salah | Cek module tree dan `use` statement |
| `type mismatch` | Return type tidak cocok | Periksa function signature dan return value |
| `borrow checker error` | Lifetime/borrow conflict | Cek apakah ada simultaneous mutable borrow |
| `linker error` | Missing system lib | `sudo apt-get install build-essential` |

---

## 2. Test Failures

### 2.1 Debugging Test yang Gagal

```bash
# Run specific test dengan output
cargo test test_name -- --nocapture

# Run test dengan logging
RUST_LOG=debug cargo test test_name -- --nocapture

# Run hanya satu file test
cargo test --test lexer_tests

# Run dengan backtrace
RUST_BACKTRACE=1 cargo test
```

### 2.2 Common Test Patterns

| Gejala | Kemungkinan Penyebab | Solusi |
|--------|----------------------|--------|
| Test timeout | Infinite loop di interpreter | Tambahkan recursion depth limit (`MAX_DEPTH = 1024`) |
| Assertion mismatch | Token kind wrong | Gunakan `dbg!(&tokens)` untuk inspect output |
| Random failures | State leak antar test | Pastikan setiap test membuat `Interpreter::new()` |
| Gradient mismatch | Numerical precision | Gunakan epsilon `1e-4`, bukan exact equality |

---

## 3. Claude Code Issues

### 3.1 Session Problems

| Masalah | Solusi |
|---------|--------|
| Claude tidak membaca CLAUDE.md | Ketik: `Read CLAUDE.md first, then proceed` |
| Claude lupa context mid-session | Re-orient: `Re-read PLANNING.md and TASKS.md, what is next?` |
| Claude membuat kode yang melanggar RULES.md | Paste rule yang dilanggar dan minta fix |
| Output terpotong | Minta: `Continue from where you left off` |
| Claude menulis ke file yang salah | Specify exact path: `Write to src/lexer/token.rs` |
| Model bukan Opus 4.6 | `claude config set model claude-opus-4-6` |

### 3.2 Best Practices dengan Claude Code

- Selalu mulai sesi dengan: Read CLAUDE.md, PLANNING.md, TASKS.md, RULES.md
- Satu task per request — jangan minta banyak hal sekaligus
- Gunakan `effort high` untuk arsitektur, `effort medium` untuk implementasi
- Jika Claude stuck, beri contoh kode dari SKILLS.md
- Review setiap commit sebelum push ke remote

---

## 4. Runtime Issues

### 4.1 Interpreter Problems

| Gejala | Diagnosa | Solusi |
|--------|----------|--------|
| Stack overflow | Rekursi tanpa base case | Cek recursion base case, tambahkan depth check |
| Wrong value type | Eval dispatch salah | Cek `eval_expr` match arms, pastikan semua Expr handled |
| Environment leak | Scope tidak di-pop | Pastikan push/pop scope balanced di block evaluation |
| Closure capture wrong | Environment parent salah | Capture `Rc::clone(&env)` saat closure dibuat |

### 4.2 Tensor Issues

| Gejala | Diagnosa | Solusi |
|--------|----------|--------|
| Shape mismatch panic | Operasi pada tensor incompatible | Cek shape sebelum operasi, gunakan `Result` |
| NaN in gradients | Learning rate terlalu besar | Kurangi lr, tambahkan gradient clipping |
| Gradient semua nol | `no_grad` context atau detached tensor | Cek `requires_grad` flag |
| Memory leak di training loop | Grad graph tidak di-clear | Panggil `zero_grad()` setiap iterasi |

---

## 5. Performance Issues

- **Slow compilation:** Gunakan `cargo check` (tanpa codegen) untuk feedback cepat
- **Slow tests:** Gunakan `cargo test --lib` (skip integration tests)
- **Slow interpreter:** Expected — tree-walking inherently slow, optimize di Phase 5
- **Slow tensor ops:** Pastikan `ndarray` menggunakan BLAS backend

```bash
# Cek BLAS availability
cargo test -- --ignored tensor_blas_benchmark

# Force BLAS
# Di Cargo.toml: ndarray = { version = "0.16", features = ["blas"] }
```

---

*Troubleshooting Version: 1.0 | Diupdate seiring development progress*
