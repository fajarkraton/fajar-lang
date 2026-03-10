/* Minimal runtime entry point for cross-compiled Fajar Lang programs.
 * Links with the Fajar object file and calls fj_main(), using its return
 * value as the process exit code. */

#include <stdio.h>

/* Fajar Lang's compiled entry function */
extern long fj_main(void);

/* Runtime: println for i64 */
void fj_rt_print_i64(long val) {
    printf("%ld\n", val);
}

/* Runtime: print for i64 (no newline) */
void fj_rt_print_i64_no_newline(long val) {
    printf("%ld", val);
}

/* Runtime: println for string (ptr + len) */
void fj_rt_println_str(const char *ptr, long len) {
    printf("%.*s\n", (int)len, ptr);
}

/* Runtime: print for string (ptr + len, no newline) */
void fj_rt_print_str(const char *ptr, long len) {
    printf("%.*s", (int)len, ptr);
}

int main(void) {
    long result = fj_main();
    return (int)result;
}
