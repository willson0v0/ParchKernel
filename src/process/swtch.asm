

.altmacro
.macro SAVE_SN n
    sd s\n, (\n+1)*8(sp)
.endm
.macro LOAD_SN n
    ld s\n, (\n+1)*8(sp)
.endm
    .section .text
    .globl __swtch
__swtch:
    # __swtch(
    #     current_context: *mut ProcessContext,
    #     next_context: *mut ProcessContext
    # )
    # push ProcessContext to current sp and save its address to where a0 points to
    
    sd    ra,   0(a0)
    sd    sp,   8(a0)
    sd    s0,  16(a0)
    sd    s1,  24(a0)
    sd    s2,  32(a0)
    sd    s3,  40(a0)
    sd    s4,  48(a0)
    sd    s5,  56(a0)
    sd    s6,  64(a0)
    sd    s7,  72(a0)
    sd    s8,  80(a0)
    sd    s9,  88(a0)
    sd   s10,  96(a0)
    sd   s11, 104(a0)
    fsd  fs0, 112(a0)
    fsd  fs1, 120(a0)
    fsd  fs2, 128(a0)
    fsd  fs3, 136(a0)
    fsd  fs4, 144(a0)
    fsd  fs5, 152(a0)
    fsd  fs6, 160(a0)
    fsd  fs7, 168(a0)
    fsd  fs8, 176(a0)
    fsd  fs9, 184(a0)
    fsd fs10, 192(a0)
    fsd fs11, 200(a0)


    ld    ra,   0(a1)
    ld    sp,   8(a1)
    ld    s0,  16(a1)
    ld    s1,  24(a1)
    ld    s2,  32(a1)
    ld    s3,  40(a1)
    ld    s4,  48(a1)
    ld    s5,  56(a1)
    ld    s6,  64(a1)
    ld    s7,  72(a1)
    ld    s8,  80(a1)
    ld    s9,  88(a1)
    ld   s10,  96(a1)
    ld   s11, 104(a1)
    fld  fs0, 112(a1)
    fld  fs1, 120(a1)
    fld  fs2, 128(a1)
    fld  fs3, 136(a1)
    fld  fs4, 144(a1)
    fld  fs5, 152(a1)
    fld  fs6, 160(a1)
    fld  fs7, 168(a1)
    fld  fs8, 176(a1)
    fld  fs9, 184(a1)
    fld fs10, 192(a1)
    fld fs11, 200(a1)
        
    ret

