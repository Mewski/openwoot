/*
 * startup.c - what runs when the microcontroller leaves reset, and where an unexpected
 * interrupt ends.
 *
 * Depends on: the linker symbols _sidata, _sdata, _edata, _sbss, _ebss (sections.ld),
 * SystemInit (system_clock.c), __libc_init_array (C library) and main (the firmware).
 */

void Reset_Handler(void);
void Default_Handler(void);

/*
 * Instruction sequence of the GigaDevice GCC startup file: copy the
 * .data load image, zero .bss, SystemInit, __libc_init_array, main. The stack pointer
 * is the one the core loaded from vector 0; nothing here needs a stack before
 * SystemInit.
 * The `bx lr` behind `bl main` is unreachable because main never returns.
 */
__attribute__((naked, noreturn))
void Reset_Handler(void)
{
    __asm__ volatile(
        "    movs    r1, #0              \n"
        "    b       2f                  \n"
        "1:  ldr     r3, =_sidata        \n" /* copy .data: *(_sdata + r1) = *(_sidata + r1) */
        "    ldr     r3, [r3, r1]        \n"
        "    str     r3, [r0, r1]        \n"
        "    adds    r1, r1, #4          \n"
        "2:  ldr     r0, =_sdata         \n"
        "    ldr     r3, =_edata         \n"
        "    adds    r2, r0, r1          \n"
        "    cmp     r2, r3              \n"
        "    bcc     1b                  \n"
        "    ldr     r2, =_sbss          \n"
        "    b       4f                  \n"
        "3:  movs    r3, #0              \n" /* zero .bss */
        "    str     r3, [r2], #4        \n"
        "4:  ldr     r3, =_ebss          \n"
        "    cmp     r2, r3              \n"
        "    bcc     3b                  \n"
        "    bl      SystemInit          \n"
        "    bl      __libc_init_array   \n"
        "    bl      main                \n"
        "    bx      lr                  \n" /* not reached */
        "    .ltorg                      \n");
}

/*
 * Target of every vector that the firmware does not claim, including all fault vectors: the
 * keyboard hangs until power is removed (no watchdog is started).
 */
void Default_Handler(void)
{
    for (;;) {
    }
}
