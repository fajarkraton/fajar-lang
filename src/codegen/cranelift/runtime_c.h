// Auto-generated Fajar Lang runtime stubs for AOT linking.
// Provides C implementations of fj_rt_* symbols that the Cranelift AOT
// compiler imports. These are linked into the final binary when `fj build`
// produces a host-target executable.
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>

// ── Print functions ────────────────────────────────────────────────
void fj_rt_print_i64(long val) { printf("%ld\n", val); }
void fj_rt_print_i64_no_newline(long val) { printf("%ld", val); }
void fj_rt_println_f64(double val) { printf("%g\n", val); }
void fj_rt_print_f64_no_newline(double val) { printf("%g", val); }
void fj_rt_println_str(const char* ptr, long len) { printf("%.*s\n", (int)len, ptr); }
void fj_rt_print_str(const char* ptr, long len) { printf("%.*s", (int)len, ptr); }
void fj_rt_println_bool(long val) { printf("%s\n", val ? "true" : "false"); }
void fj_rt_print_bool(long val) { printf("%s", val ? "true" : "false"); }

// ── Debug / Error print ────────────────────────────────────────────
void fj_rt_dbg_i64(long val) { fprintf(stderr, "[dbg] %ld\n", val); }
void fj_rt_dbg_str(const char* ptr, long len) { fprintf(stderr, "[dbg] %.*s\n", (int)len, ptr); }
void fj_rt_dbg_f64(double val) { fprintf(stderr, "[dbg] %g\n", val); }
void fj_rt_eprintln_i64(long val) { fprintf(stderr, "%ld\n", val); }
void fj_rt_eprintln_str(const char* ptr, long len) { fprintf(stderr, "%.*s\n", (int)len, ptr); }
void fj_rt_eprintln_f64(double val) { fprintf(stderr, "%g\n", val); }
void fj_rt_eprintln_bool(long val) { fprintf(stderr, "%s\n", val ? "true" : "false"); }
void fj_rt_eprint_i64(long val) { fprintf(stderr, "%ld", val); }
void fj_rt_eprint_str(const char* ptr, long len) { fprintf(stderr, "%.*s", (int)len, ptr); }

// ── Memory ─────────────────────────────────────────────────────────
void* fj_rt_alloc(long size) { return malloc((size_t)size); }
void fj_rt_free(void* ptr) { free(ptr); }

// ── String parsing ─────────────────────────────────────────────────
long fj_rt_parse_int(const char* ptr, long len) {
    char buf[64];
    long n = len < 63 ? len : 63;
    memcpy(buf, ptr, (size_t)n);
    buf[n] = '\0';
    return atol(buf);
}
double fj_rt_parse_float(const char* ptr, long len) {
    char buf[64];
    long n = len < 63 ? len : 63;
    memcpy(buf, ptr, (size_t)n);
    buf[n] = '\0';
    return atof(buf);
}

// ── String operations ──────────────────────────────────────────────
long fj_rt_str_len(const char* ptr, long len) { (void)ptr; return len; }
long fj_rt_str_eq(const char* a, long al, const char* b, long bl) {
    return al == bl && memcmp(a, b, (size_t)al) == 0;
}

// ── Math ───────────────────────────────────────────────────────────
double fj_rt_sqrt(double x) { return sqrt(x); }
double fj_rt_sin(double x) { return sin(x); }
double fj_rt_cos(double x) { return cos(x); }
double fj_rt_tan(double x) { return tan(x); }
double fj_rt_floor(double x) { return floor(x); }
double fj_rt_ceil(double x) { return ceil(x); }
double fj_rt_round(double x) { return round(x); }
double fj_rt_abs_f64(double x) { return fabs(x); }
long fj_rt_abs_i64(long x) { return x < 0 ? -x : x; }
double fj_rt_pow(double base, double exp) { return pow(base, exp); }
double fj_rt_log(double x) { return log(x); }
double fj_rt_exp(double x) { return exp(x); }

// ── Process control ────────────────────────────────────────────────
void fj_rt_exit(long code) { exit((int)code); }
void fj_rt_panic(const char* ptr, long len) {
    fprintf(stderr, "panic: %.*s\n", (int)len, ptr);
    exit(1);
}
void fj_rt_assert_fail(const char* ptr, long len) {
    fprintf(stderr, "assertion failed: %.*s\n", (int)len, ptr);
    exit(1);
}

// ── Format (stub) ──────────────────────────────────────────────────
// fj_rt_format is complex (variadic formatting). Provide a minimal stub.
long fj_rt_format(const char* fmt, long fmt_len, char* out, long out_cap) {
    long n = fmt_len < out_cap ? fmt_len : out_cap;
    memcpy(out, fmt, (size_t)n);
    return n;
}
