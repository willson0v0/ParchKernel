

use crate::{mem::{VirtAddr, PhysAddr}, config::TRAP_CONTEXT_ADDR};


#[repr(C)]
#[derive(Clone, Debug)]
pub struct TrapContext {
    pub kernel_sp   : VirtAddr,     /*   0 */   // used by kernel
    pub user_trap   : VirtAddr,     /*   8 */   // used by kernel
    pub epc         : VirtAddr,     /*  16 */   // used by kernel
    pub hart_id     : usize,        /*  24 */   // used by kernel
    pub ra          : usize,        /*  32 */
    pub sp          : usize,        /*  40 */
    pub gp          : usize,        /*  48 */
    pub tp          : usize,        /*  56 */
    pub t0          : usize,        /*  64 */
    pub t1          : usize,        /*  72 */
    pub t2          : usize,        /*  80 */
    pub s0          : usize,        /*  88 */
    pub s1          : usize,        /*  96 */
    pub a0          : usize,        /* 104 */
    pub a1          : usize,        /* 112 */
    pub a2          : usize,        /* 120 */
    pub a3          : usize,        /* 128 */
    pub a4          : usize,        /* 136 */
    pub a5          : usize,        /* 144 */
    pub a6          : usize,        /* 152 */
    pub a7          : usize,        /* 160 */
    pub s2          : usize,        /* 168 */
    pub s3          : usize,        /* 176 */
    pub s4          : usize,        /* 184 */
    pub s5          : usize,        /* 192 */
    pub s6          : usize,        /* 200 */
    pub s7          : usize,        /* 208 */
    pub s8          : usize,        /* 216 */
    pub s9          : usize,        /* 224 */
    pub s10         : usize,        /* 232 */
    pub s11         : usize,        /* 240 */
    pub t3          : usize,        /* 248 */
    pub t4          : usize,        /* 256 */
    pub t5          : usize,        /* 264 */
    pub t6          : usize,        /* 272 */
    pub ft0         : f64,          /* 280 */
    pub ft1         : f64,          /* 288 */
    pub ft2         : f64,          /* 296 */
    pub ft3         : f64,          /* 304 */
    pub ft4         : f64,          /* 312 */
    pub ft5         : f64,          /* 320 */
    pub ft6         : f64,          /* 328 */
    pub ft7         : f64,          /* 336 */
    pub fs0         : f64,          /* 344 */
    pub fs1         : f64,          /* 352 */
    pub fa0         : f64,          /* 360 */
    pub fa1         : f64,          /* 368 */
    pub fa2         : f64,          /* 376 */
    pub fa3         : f64,          /* 384 */
    pub fa4         : f64,          /* 392 */
    pub fa5         : f64,          /* 400 */
    pub fa6         : f64,          /* 408 */
    pub fa7         : f64,          /* 416 */
    pub fs2         : f64,          /* 424 */
    pub fs3         : f64,          /* 432 */
    pub fs4         : f64,          /* 440 */
    pub fs5         : f64,          /* 448 */
    pub fs6         : f64,          /* 456 */
    pub fs7         : f64,          /* 464 */
    pub fs8         : f64,          /* 472 */
    pub fs9         : f64,          /* 480 */
    pub fs10        : f64,          /* 488 */
    pub fs11        : f64,          /* 496 */
    pub ft8         : f64,          /* 504 */
    pub ft9         : f64,          /* 512 */
    pub ft10        : f64,          /* 520 */
    pub ft11        : f64,          /* 528 */
}

impl TrapContext {
    pub fn new() -> Self {
        unsafe {
            core::mem::zeroed()
        }
    }
    
    pub fn current_ref() -> &'static mut TrapContext {
        unsafe {(TRAP_CONTEXT_ADDR.0 as * mut TrapContext).as_mut().unwrap()}
    }

    pub unsafe fn from_pa(pa: PhysAddr) -> &'static mut TrapContext {
        (pa.0 as * mut TrapContext).as_mut().unwrap()
    }
}