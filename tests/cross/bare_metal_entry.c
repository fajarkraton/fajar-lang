/* Bare-metal-style entry point for QEMU user-mode testing.
 * Tests the bare-metal compilation pipeline without requiring
 * a full bare-metal runtime. The Fajar main returns an exit code. */

#include <stdio.h>

extern long main(void);

/* Simulated UART output for bare-metal testing */
static char uart_buffer[256];
static int uart_pos = 0;

void fj_rt_print_i64(long val) {
    printf("%ld\n", val);
}

void fj_rt_print_i64_no_newline(long val) {
    printf("%ld", val);
}

/* Simulated port_write for UART testing */
void port_write(long addr, long val) {
    if (addr == 0x09000000 && uart_pos < 255) {
        uart_buffer[uart_pos++] = (char)val;
    }
}

int _start(void) {
    long result = main();
    /* Print what was written to UART */
    uart_buffer[uart_pos] = '\0';
    if (uart_pos > 0) {
        printf("UART: %s", uart_buffer);
    }
    return (int)result;
}
