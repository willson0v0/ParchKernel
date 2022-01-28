// TODO: Needed for signal handling

    .section .text.u_trampoline
    .globl __user_call_sigreturn
    .globl __user_restore_from_handler

# run in user mode
__user_restore_from_handler:
    # restore stack pointer. This now should point directly at head of a TrapContext, the one saved by __restore_to_signal_handler
    # addi sp, sp, 36*8
    # not returning to protect the stack. moved to sigreturn.
    # syscall num for sys_sigreturn: 
    li a7, 139
    ecall

.align 4
__siginfo:
    .space 64