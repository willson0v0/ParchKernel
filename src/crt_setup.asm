    .section .text.entry
    .globl _start
_start:
    la sp, boot_stack
    li t0, 4096 * 4
    csrr tp, mhartid
    addi t1, tp, 1
    mul t0, t0, t1
    add sp, sp, t0
    csrr a0, mhartid
    call genesis_m

    .section .bss.stack
    .globl boot_stack
boot_stack:
    .space 4096 * 4 * 16
    .globl boot_stack_top
boot_stack_top: