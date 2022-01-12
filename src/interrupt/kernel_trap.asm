# THESE PART IS FROM XV6, SO HERE'S THE ORIGINAL MIT LICENSE HERE FOR YA'll

# The xv6 software is:
# 
# Copyright (c) 2006-2019 Frans Kaashoek, Robert Morris, Russ Cox,
#                         Massachusetts Institute of Technology
# 
# Permission is hereby granted, free of charge, to any person obtaining
# a copy of this software and associated documentation files (the
# "Software"), to deal in the Software without restriction, including
# without limitation the rights to use, copy, modify, merge, publish,
# distribute, sublicense, and/or sell copies of the Software, and to
# permit persons to whom the Software is furnished to do so, subject to
# the following conditions:
# 
# The above copyright notice and this permission notice shall be
# included in all copies or substantial portions of the Software.
# 
# THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
# EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
# MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
# NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE
# LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
# OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION
# WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

	#
        # interrupts and exceptions while in supervisor
        # mode come here.
        #
        # push all registers, call kerneltrap(), restore, return.
        #
.section .text
.globl kernel_trap
.globl kernel_vec
.align 4
kernel_vec:
	// make room to save registers.
	addi sp, sp, -256

	// save the registers.
	sd ra, 0(sp)
	sd sp, 8(sp)
	sd gp, 16(sp)
	sd tp, 24(sp)
	sd t0, 32(sp)
	sd t1, 40(sp)
	sd t2, 48(sp)
	sd s0, 56(sp)
	sd s1, 64(sp)
	sd a0, 72(sp)
	sd a1, 80(sp)
	sd a2, 88(sp)
	sd a3, 96(sp)
	sd a4, 104(sp)
	sd a5, 112(sp)
	sd a6, 120(sp)
	sd a7, 128(sp)
	sd s2, 136(sp)
	sd s3, 144(sp)
	sd s4, 152(sp)
	sd s5, 160(sp)
	sd s6, 168(sp)
	sd s7, 176(sp)
	sd s8, 184(sp)
	sd s9, 192(sp)
	sd s10, 200(sp)
	sd s11, 208(sp)
	sd t3, 216(sp)
	sd t4, 224(sp)
	sd t5, 232(sp)
	sd t6, 240(sp)

// call the rust trap handler in trap_handler.asm
	call kernel_trap

	// restore registers.
	ld ra, 0(sp)
	ld sp, 8(sp)
	ld gp, 16(sp)
	// not this, in case we moved CPUs: ld tp, 24(sp)
	ld t0, 32(sp)
	ld t1, 40(sp)
	ld t2, 48(sp)
	ld s0, 56(sp)
	ld s1, 64(sp)
	ld a0, 72(sp)
	ld a1, 80(sp)
	ld a2, 88(sp)
	ld a3, 96(sp)
	ld a4, 104(sp)
	ld a5, 112(sp)
	ld a6, 120(sp)
	ld a7, 128(sp)
	ld s2, 136(sp)
	ld s3, 144(sp)
	ld s4, 152(sp)
	ld s5, 160(sp)
	ld s6, 168(sp)
	ld s7, 176(sp)
	ld s8, 184(sp)
	ld s9, 192(sp)
	ld s10, 200(sp)
	ld s11, 208(sp)
	ld t3, 216(sp)
	ld t4, 224(sp)
	ld t5, 232(sp)
	ld t6, 240(sp)

	addi sp, sp, 256

	// return to whatever we were doing in the kernel.
	sret

.globl timervec
.align 4
timervec:
	# start.c has set up the memory that mscratch points to:
	# scratch[0,8,16] : register save area.
	# scratch[32] : address of CLINT's MTIMECMP register.
	# scratch[40] : desired interval between interrupts.
	
	csrrw a0, mscratch, a0
	sd a1, 0(a0)
	sd a2, 8(a0)
	sd a3, 16(a0)

	# schedule the next timer interrupt
	# by adding interval to mtimecmp.
	ld a1, 32(a0) # CLINT_MTIMECMP(hart)
	ld a2, 40(a0) # interval
	ld a3, 0(a1)
	add a3, a3, a2
	sd a3, 0(a1)

	# raise a supervisor software interrupt.
	li a1, 2
	csrw sip, a1

	ld a3, 16(a0)
	ld a2, 8(a0)
	ld a1, 0(a0)
	csrrw a0, mscratch, a0

	mret
