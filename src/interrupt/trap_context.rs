use crate::{mem::VirtAddr, config::TRAP_CONTEXT_ADDR};


#[repr(C)]
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
}

impl TrapContext {
    pub fn get_ref() -> &'static mut TrapContext {
        unsafe {(TRAP_CONTEXT_ADDR.0 as * mut TrapContext).as_mut().unwrap()}
    }
}