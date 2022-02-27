# THIS PART IS FROM XV6, SO HERE'S THE ORIGINAL MIT LICENSE HERE FOR YA'll

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
	# code to switch between user and kernel space.
	#
	# this code is mapped at the same virtual address
	# (TRAMPOLINE) in user and kernel space so that
	# it continues to work when it switches page tables.
#
# kernel.ld causes this to be aligned
	# to a page boundary.
	#
.section .text.trampoline
.globl trampoline
trampoline:
.align 4
.globl uservec
uservec:    
	#
        # trap.c sets stvec to point here, so
        # traps from user space start here,
        # in supervisor mode, but with a
        # user page table.
        #
        # sscratch points to where the process's p->trapframe is
        # mapped into user space, at TRAPFRAME.
        #
        
	# swap a0 and sscratch
        # so that a0 is TRAPFRAME
        csrrw a0, sscratch, a0

        # save the user registers in TRAPFRAME
        sd ra,   32(a0)
        sd sp,   40(a0)
        sd gp,   48(a0)
        sd tp,   56(a0)
        sd t0,   64(a0)
        sd t1,   72(a0)
        sd t2,   80(a0)
        sd s0,   88(a0)
        sd s1,   96(a0)
        sd a1,  112(a0)
        sd a2,  120(a0)
        sd a3,  128(a0)
        sd a4,  136(a0)
        sd a5,  144(a0)
        sd a6,  152(a0)
        sd a7,  160(a0)
        sd s2,  168(a0)
        sd s3,  176(a0)
        sd s4,  184(a0)
        sd s5,  192(a0)
        sd s6,  200(a0)
        sd s7,  208(a0)
        sd s8,  216(a0)
        sd s9,  224(a0)
        sd s10, 232(a0)
        sd s11, 240(a0)
        sd t3,  248(a0)
        sd t4,  256(a0)
        sd t5,  264(a0)
        sd t6,  272(a0)
        
        fsd ft0 , 280(a0)
        fsd ft1 , 288(a0)
        fsd ft2 , 296(a0)
        fsd ft3 , 304(a0)
        fsd ft4 , 312(a0)
        fsd ft5 , 320(a0)
        fsd ft6 , 328(a0)
        fsd ft7 , 336(a0)
        fsd fs0 , 344(a0)
        fsd fs1 , 352(a0)
        fsd fa0 , 360(a0)
        fsd fa1 , 368(a0)
        fsd fa2 , 376(a0)
        fsd fa3 , 384(a0)
        fsd fa4 , 392(a0)
        fsd fa5 , 400(a0)
        fsd fa6 , 408(a0)
        fsd fa7 , 416(a0)
        fsd fs2 , 424(a0)
        fsd fs3 , 432(a0)
        fsd fs4 , 440(a0)
        fsd fs5 , 448(a0)
        fsd fs6 , 456(a0)
        fsd fs7 , 464(a0)
        fsd fs8 , 472(a0)
        fsd fs9 , 480(a0)
        fsd fs10, 488(a0)
        fsd fs11, 496(a0)
        fsd ft8 , 504(a0)
        fsd ft9 , 512(a0)
        fsd ft10, 520(a0)
        fsd ft11, 528(a0)

	# save the user a0 in p->trapframe->a0
        csrr t0, sscratch
        sd t0, 104(a0)

        # restore kernel stack pointer from p->trapframe->kernel_sp
        ld sp, 0(a0)

        # make tp hold the current hartid, from p->trapframe->kernel_hartid
        ld tp, 24(a0)

        # load the address of usertrap(), p->trapframe->kernel_trap
        ld t0, 8(a0)

        # not switching pagetable
        # ld t1, 0(a0)
        # csrw satp, t1
        # sfence.vma zero, zero

        # a0 is no longer valid, since the kernel page
        # table does not specially map p->tf.

        # jump to usertrap(), which does not return
        jr t0

.globl userret
userret:
        # userret(TRAPFRAME: VirtAddr)
        # switch from kernel to user.
        # usertrapret() calls here.
        # a0: TRAPFRAME
        # Sharing pagetable between kernel thread and user, not switching pagetable

        # put the saved user a0 in sscratch, so we
        # can swap it with our a0 (TRAPFRAME) in the last step.
        ld t0, 104(a0)
        csrw sscratch, t0

        # restore all but a0 from TRAPFRAME
        ld ra,   32(a0)
        ld sp,   40(a0)
        ld gp,   48(a0)
        ld tp,   56(a0)
        ld t0,   64(a0)
        ld t1,   72(a0)
        ld t2,   80(a0)
        ld s0,   88(a0)
        ld s1,   96(a0)
        ld a1,  112(a0)
        ld a2,  120(a0)
        ld a3,  128(a0)
        ld a4,  136(a0)
        ld a5,  144(a0)
        ld a6,  152(a0)
        ld a7,  160(a0)
        ld s2,  168(a0)
        ld s3,  176(a0)
        ld s4,  184(a0)
        ld s5,  192(a0)
        ld s6,  200(a0)
        ld s7,  208(a0)
        ld s8,  216(a0)
        ld s9,  224(a0)
        ld s10, 232(a0)
        ld s11, 240(a0)
        ld t3,  248(a0)
        ld t4,  256(a0)
        ld t5,  264(a0)
        ld t6,  272(a0)

        fld ft0 , 280(a0)
        fld ft1 , 288(a0)
        fld ft2 , 296(a0)
        fld ft3 , 304(a0)
        fld ft4 , 312(a0)
        fld ft5 , 320(a0)
        fld ft6 , 328(a0)
        fld ft7 , 336(a0)
        fld fs0 , 344(a0)
        fld fs1 , 352(a0)
        fld fa0 , 360(a0)
        fld fa1 , 368(a0)
        fld fa2 , 376(a0)
        fld fa3 , 384(a0)
        fld fa4 , 392(a0)
        fld fa5 , 400(a0)
        fld fa6 , 408(a0)
        fld fa7 , 416(a0)
        fld fs2 , 424(a0)
        fld fs3 , 432(a0)
        fld fs4 , 440(a0)
        fld fs5 , 448(a0)
        fld fs6 , 456(a0)
        fld fs7 , 464(a0)
        fld fs8 , 472(a0)
        fld fs9 , 480(a0)
        fld fs10, 488(a0)
        fld fs11, 496(a0)
        fld ft8 , 504(a0)
        fld ft9 , 512(a0)
        fld ft10, 520(a0)
        fld ft11, 528(a0)

	# restore user a0, and save TRAPFRAME in sscratch
        csrrw a0, sscratch, a0
        
        # return to user mode and user pc.
        # usertrapret() set up sstatus and sepc.
        sret
