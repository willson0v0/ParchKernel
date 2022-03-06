#[macro_export]
macro_rules! CALL_SYSCALL {
    ( $do_trace: expr, $syscall_name: expr ) => {
        {
            let pid = crate::process::get_processor().current().unwrap().pid;
            // don't hold process because sys_exit might not return.
            if $do_trace {
                info!("SYSCALL {} CALLED BY {:?}", stringify!($syscall_name), pid);
            }
            let ret = $syscall_name();
            if $do_trace {
                info!("SYSCALL {} CALLED BY {:?} RESULT {:?}", stringify!($syscall_name), pid, ret);
            }
            ret
        }
    };
    ( $do_trace: expr, $syscall_name: expr, $($y:expr),+ ) => {
        {
            let pid = crate::process::get_processor().current().unwrap().pid;
            if $do_trace {
                info!("SYSCALL {} CALLED BY {:?}", stringify!($syscall_name), pid);
                $(
                    debug!("{:>25} = {:?}", stringify!{$y}, $y);
                )+
            }
            let ret = $syscall_name($($y),+);
            if $do_trace {
                info!("SYSCALL {} CALLED BY {:?} RESULT {:?}", stringify!($syscall_name), pid, ret);
            }
            ret
        }
    };
}

#[macro_export]
macro_rules! enum_with_tryfrom_usize {
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
macro_rules! enum_with_tryfrom_u16 {
    ($(#[$meta:meta])* $vis:vis enum $name:ident {
        $($(#[$vmeta:meta])* $vname:ident $(= $val:expr)?,)*
    }) => {
        $(#[$meta])*
        $vis enum $name {
            $($(#[$vmeta])* $vname $(= $val)?,)*
        }

        impl core::convert::TryFrom<u16> for $name {
            type Error = crate::utils::ErrorNum;

            fn try_from(v: u16) -> Result<Self, Self::Error> {
                match v {
                    $(x if x == $name::$vname as u16 => Ok($name::$vname),)*
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