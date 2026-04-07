# Phase 3.6+3.7: XMM/YMM Register Support — Detailed Plan

> **Goal:** Enable AVX2 SIMD + AES-NI in FajarOS via LLVM inline asm
> **Approach:** Memory-based operands (addresses → inline asm loads/stores internally)
> **No language changes needed** — asm templates manage their own XMM registers

## Background

Current LLVM inline asm (`compile_inline_asm()` at line 7145) assumes all operands
are i64. XMM/YMM operations need 128/256-bit values. Rather than adding vector types
to the language, we use memory-based operands: pass addresses as i64, and the asm
template handles `movdqa`/`vmovdqu` internally.

## Phase 3.6: AVX2 Tensor SIMD (6 tasks)

### Task 3.6.1: avx2_dot_f32(a_ptr, b_ptr, len) -> f32
**File:** `src/codegen/llvm/mod.rs` — add to `compile_builtin_call()`
**Template:**
```asm
vxorps %ymm0, %ymm0, %ymm0        # accumulator = 0
movq %rdi, %rax                     # a_ptr
movq %rsi, %rbx                     # b_ptr
movq %rdx, %rcx                     # len
shrq $3, %rcx                       # len/8 (8 floats per ymm)
.L_avx_loop:
testq %rcx, %rcx
jz .L_avx_done
vmovups (%rax), %ymm1
vmovups (%rbx), %ymm2
vfmadd231ps %ymm2, %ymm1, %ymm0    # acc += a * b
addq $32, %rax
addq $32, %rbx
decq %rcx
jmp .L_avx_loop
.L_avx_done:
# Horizontal sum ymm0 → scalar
vextractf128 $1, %ymm0, %xmm1
vaddps %xmm1, %xmm0, %xmm0
vhaddps %xmm0, %xmm0, %xmm0
vhaddps %xmm0, %xmm0, %xmm0
vmovss %xmm0, (%rsp)
movl (%rsp), %eax                   # result bits in eax
vzeroupper
```
**Constraint:** `"={eax},{rdi},{rsi},{rdx},~{rax},~{rbx},~{rcx},~{ymm0},~{ymm1},~{ymm2},~{xmm0},~{xmm1}"`
**Verify:** `avx2_dot_f32(a_ptr, b_ptr, 8)` returns correct dot product

### Task 3.6.2: avx2_add_f32(dst_ptr, a_ptr, b_ptr, len)
**File:** `src/codegen/llvm/mod.rs`
**Template:** Loop with `vmovups` + `vaddps` + `vmovups` store
**Constraint:** Address-based (all i64 pointers)
**Verify:** Element-wise add of f32 arrays

### Task 3.6.3: avx2_mul_f32(dst_ptr, a_ptr, b_ptr, len)
**Template:** Loop with `vmovups` + `vmulps` + `vmovups` store

### Task 3.6.4: avx2_relu_f32(dst_ptr, src_ptr, len)
**Template:** `vmaxps %ymm_zero, %ymm_src, %ymm_dst`

### Task 3.6.5: Wire AVX2 builtins to interpreter
**File:** `src/interpreter/eval/builtins.rs`
**Change:** Add `avx2_dot_f32`, `avx2_add_f32`, `avx2_mul_f32`, `avx2_relu_f32`
**Fallback:** If not compiled with LLVM backend, use scalar CPU path

### Task 3.6.6: Tests
**File:** `src/codegen/llvm/mod.rs` — test module
**Tests:** avx2_dot_f32 correctness, avx2_add element-wise, avx2_relu

## Phase 3.7: AES-NI Crypto (5 tasks)

### Task 3.7.1: aesni_encrypt_block(state_ptr, key_ptr, rounds) -> writes to state_ptr
**File:** `src/codegen/llvm/mod.rs` — add to `compile_builtin_call()`
**Template:**
```asm
movdqu (%rdi), %xmm0               # state = *state_ptr
movdqu (%rsi), %xmm1               # round_key = *key_ptr
pxor %xmm1, %xmm0                  # initial XOR
# 9 rounds of aesenc (AES-128)
movdqu 16(%rsi), %xmm1
aesenc %xmm1, %xmm0
movdqu 32(%rsi), %xmm1
aesenc %xmm1, %xmm0
... (repeat for rounds 2-8)
movdqu 160(%rsi), %xmm1
aesenclast %xmm1, %xmm0
movdqu %xmm0, (%rdi)               # *state_ptr = encrypted
```
**Constraint:** `"{rdi},{rsi},{rdx},~{xmm0},~{xmm1}"`
**Verify:** Compare against known AES-128 test vector (NIST FIPS 197)

### Task 3.7.2: aesni_decrypt_block(state_ptr, key_ptr, rounds)
**Template:** `aesdec` + `aesdeclast` sequence
**Verify:** encrypt then decrypt = original

### Task 3.7.3: aesni_key_expand(key_ptr, expanded_ptr, key_bits)
**Template:** `aeskeygenassist` + shift/xor sequence
**Verify:** Key expansion matches NIST test vectors

### Task 3.7.4: Wire AES-NI builtins to interpreter
**File:** `src/interpreter/eval/builtins.rs`
**Change:** Add `aesni_encrypt_block`, `aesni_decrypt_block`, `aesni_key_expand`
**Fallback:** Software AES when AES-NI not available

### Task 3.7.5: Tests + NIST validation
**File:** `src/codegen/llvm/mod.rs` — test module
**Tests:** FIPS 197 Appendix B test vector, encrypt/decrypt roundtrip

## Execution Order

```
3.6.1 (dot_f32) → 3.6.2 (add) → 3.6.3 (mul) → 3.6.4 (relu)
                                                 → 3.6.5 (wire)
                                                 → 3.6.6 (tests)
3.7.1 (encrypt) → 3.7.2 (decrypt) → 3.7.3 (key_expand)
                                    → 3.7.4 (wire)
                                    → 3.7.5 (tests)
```

## Key Insight: No Vector Type Changes Needed

The LLVM constraint system already handles `{xmm0}`, `{ymm0}` etc. (line 6919
wraps any register name in braces). By using memory-based operands (pass addresses
as i64, asm loads/stores internally), we avoid needing vector types in the type system.

This matches how Linux kernel and FajarOS currently handle SIMD: data lives in memory,
asm blocks manage their own register allocation.
