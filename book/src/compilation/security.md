# Security Hardening

Fajar Lang provides compiler-level security features to harden binaries against common attack vectors.

## Stack Protection

### Stack Canaries

```bash
fj build -fharden examples/server.fj
```

Inserts random canary values before return addresses. Buffer overflows corrupt the canary, which is detected before the function returns.

### Shadow Stack

Stores return addresses in a separate shadow stack. Even if the main stack is corrupted, the shadow stack provides the correct return address.

### Stack Clash Protection

Inserts guard page probes to prevent stack clash attacks where the stack grows into the heap.

## Control-Flow Integrity (CFI)

### Forward-Edge CFI

Validates indirect call targets using type hashes. Only functions with matching signatures can be called through function pointers.

### Backward-Edge CFI

Validates return addresses using the shadow stack.

### VTable Guards

Validates vtable pointers for trait object dispatch, preventing vtable hijacking.

## Memory Sanitizers

### AddressSanitizer (ASan)

Detects buffer overflows, use-after-free, and double-free:

```bash
fj build --asan examples/program.fj
```

Uses shadow memory and red zones around allocations.

### MemorySanitizer (MSan)

Detects reads of uninitialized memory.

### Leak Detector

Reports memory leaks at program exit with allocation site information.

## Binary Hardening

| Feature | Flag | Protection |
|---------|------|------------|
| Stack canaries | `-fharden` | Buffer overflow |
| CFI | `-fharden` | ROP/JOP attacks |
| PIC | `-fpic` | ASLR compatibility |
| RELRO | `-fharden` | GOT overwrite |
| NX stack | Default | Code injection |
| FORTIFY | `-fharden` | Format string attacks |

## Security Audit

```bash
fj audit
```

Produces a security score (0-100) analyzing the binary for missing hardening features.
