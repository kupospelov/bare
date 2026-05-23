use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Debug,
    Info,
    Warning,
    Error,
}

static LEVEL: OnceLock<Level> = OnceLock::new();

pub fn get() -> Level {
    LEVEL.get().copied().unwrap()
}

pub fn set(level: Level) {
    let _ = LEVEL.set(level);
}

#[macro_export]
macro_rules! log {
    ($level:expr, $($arg:tt)*) => {
        if $level >= $crate::log::get() {
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
