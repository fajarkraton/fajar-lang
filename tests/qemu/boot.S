/* Minimal multiboot kernel for QEMU boot test.
 * Writes "FAJAR-BOOT-OK" to serial port 0x3F8, then exits.
 */

.set MAGIC, 0x1BADB002
.set FLAGS, 0x0
.set CHECKSUM, -(MAGIC + FLAGS)

.section .multiboot
.align 4
.long MAGIC
.long FLAGS
.long CHECKSUM

.section .text
.global _start
_start:
    /* Init serial: 115200 8N1 */
    mov $0x3F9, %dx
    mov $0x00, %al
    outb %al, %dx
    mov $0x3FB, %dx
    mov $0x80, %al
    outb %al, %dx
    mov $0x3F8, %dx
    mov $0x01, %al
    outb %al, %dx
    mov $0x3F9, %dx
    mov $0x00, %al
    outb %al, %dx
    mov $0x3FB, %dx
    mov $0x03, %al
    outb %al, %dx
    mov $0x3FC, %dx
    mov $0x00, %al
    outb %al, %dx

    /* Print each character */
    mov $msg, %esi
.Lloop:
    movb (%esi), %al
    test %al, %al
    jz .Lexit
    /* Wait for TX ready */
.Lwait:
    mov $0x3FD, %dx
    inb %dx, %al
    test $0x20, %al
    jz .Lwait
    /* Send byte */
    movb (%esi), %al
    mov $0x3F8, %dx
    outb %al, %dx
    inc %esi
    jmp .Lloop

.Lexit:
    /* Exit QEMU: debug-exit port */
    mov $0xF4, %dx
    mov $0x00, %al
    outb %al, %dx
    cli
    hlt

.section .rodata
msg:
    .asciz "FAJAR-BOOT-OK\n"
