macro_rules! enum_with_tryfrom {
    ($(#[$meta:meta])* $vis:vis enum $name:ident {
        $($(#[$vmeta:meta])* $vname:ident $(= $val:expr)?,)*
    }) => {
        $(#[$meta])*
        $vis enum $name {
            $($(#[$vmeta])* $vname $(= $val)?,)*
        }

        impl core::convert::TryFrom<usize> for $name {
            type Error = crate::utils::ErrorNum;

            fn try_from(v: usize) -> Result<Self, Self::Error> {
                match v {
                    $(x if x == $name::$vname as usize => Ok($name::$vname),)*
                    _ => Err(crate::utils::ErrorNum::ENOSYS),
                }
            }
        }
    }
}

enum_with_tryfrom!{
    #[repr(usize)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
    pub enum SignalNum {
        SIGHUP    =  1,
        SIGINT    =  2,
        SIGQUIT   =  3,
        SIGILL    =  4,
        SIGTRAP   =  5,
        SIGABRT   =  6,
        SIGBUS    =  7,
        SIGFPE    =  8,
        SIGKILL   =  9,
        SIGUSR1   = 10,
        SIGSEGV   = 11,
        SIGUSR2   = 12,
        SIGPIPE   = 13,
        SIGALRM   = 14,
        SIGTERM   = 15,
        SIGSTKFLT = 16,
        SIGCHLD   = 17,
        SIGCONT   = 18,
        SIGSTOP   = 19,
        SIGTSTP   = 20,
        SIGTTIN   = 21,
        SIGTTOU   = 22,
        SIGURG    = 23,
        SIGXCPU   = 24,
        SIGXFSZ   = 25,
        SIGVTALRM = 26,
        SIGPROF   = 27,
        SIGWINCH  = 28,
        SIGIO     = 29,
        SIGPWR    = 30,
        SIGSYS    = 31,
	}
}

impl core::fmt::Display for SignalNum {
	fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
		core::fmt::Debug::fmt(self, f)
	}
}