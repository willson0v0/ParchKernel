#[macro_export]
macro_rules! CALL_SYSCALL {
    ( $syscall_name: expr ) => {
        {
            let process = crate::process::get_processor().current().unwrap();
            let do_trace = process.get_inner().trace;
            if do_trace {
                debug!("/========== SYSCALL {} CALLED BY {:?} ==========\\", stringify!($syscall_name), process.pid);
            }
            let ret = $syscall_name();
            if do_trace {
                debug!("\\= SYSCALL {} CALLED BY {} RESULT {:<10?} =/", stringify!($syscall_name), process.pid, ret);
            }
            ret
        }
    };
    ( $syscall_name: expr, $($y:expr),+ ) => {
        {
            let process = crate::process::get_processor().current().unwrap();
            let do_trace = process.get_inner().trace;
            if do_trace {
                debug!("SYSCALL {} CALLED BY {:?}", stringify!($syscall_name), process.pid);
                $(
                    verbose!("{:>25} = {:?}", stringify!{$y}, $y);
                )+
            }
            let ret = $syscall_name($($y),+);
            if do_trace {
                debug!("SYSCALL {} CALLED BY {:?} RESULT {:?}", stringify!($syscall_name), process.pid, ret);
            }
            ret
        }
    };
}

#[macro_export]
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


#[macro_export]
macro_rules! verbose {
    ($($arg:tt)*) => {
        $crate::utils::log($crate::utils::LogLevel::Verbose, format_args!($($arg)*))
    }
}

#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => {
        $crate::utils::log($crate::utils::LogLevel::Debug, format_args!($($arg)*))
    }
}

#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {
        $crate::utils::log($crate::utils::LogLevel::Info, format_args!($($arg)*))
    }
}

#[macro_export]
macro_rules! warning {
    ($($arg:tt)*) => {
        $crate::utils::log($crate::utils::LogLevel::Warning, format_args!($($arg)*))
    }
}

#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {
        $crate::utils::log($crate::utils::LogLevel::Error, format_args!($($arg)*))
    }
}

#[macro_export]
macro_rules! milestone {
    ($($arg:tt)*) => {
        $crate::utils::log($crate::utils::LogLevel::Milestone, format_args!($($arg)*))
    }
}

#[macro_export]
macro_rules! fatal {
    ($($arg:tt)*) => {
        $crate::utils::log($crate::utils::LogLevel::Fatal, format_args!($($arg)*))
    }
}

#[macro_export]
macro_rules! log {
    ($lvl:tt, $($arg:tt)*) => {
        $crate::utils::log($lvl, format_args!($($arg)*));
    }
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        $crate::utils::print(format_args!($($arg)*))
    }
}

#[macro_export]
macro_rules! print_no_lock {
    ($($arg:tt)*) => {
        $crate::utils::print_no_lock(format_args!($($arg)*))
    }
}

/// The great println! macro. Prints to the standard output. Also prints a linefeed (`\\n`, or U+000A).
#[macro_export]
macro_rules! println {
    () => {
        $crate::print!("\r\n")
    };
    
    ($($arg:tt)*) => {
        $crate::print!("{}\r\n", format_args!($($arg)*))
    };
}