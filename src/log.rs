#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Debug,
    Info,
    Warning,
    Error,
}

#[macro_export]
macro_rules! log {
    ($level:expr, $($arg:tt)*) => {
        if $level >= $crate::LOG_LEVEL {
            eprintln!("[{:?}] {}", $level, format_args!($($arg)*));
        }
    };
}

#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => {
        $crate::log!($crate::log::Level::Debug, $($arg)*)
    };
}

#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {
        $crate::log!($crate::log::Level::Info, $($arg)*)
    };
}

#[macro_export]
macro_rules! warning {
    ($($arg:tt)*) => {
        $crate::log!($crate::log::Level::Warning, $($arg)*)
    };
}

#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {
        $crate::log!($crate::log::Level::Error, $($arg)*)
    };
}
