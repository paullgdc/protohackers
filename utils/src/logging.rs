#[macro_export]
macro_rules! log {
    ($fmt_str:expr $(, $arg:expr)* $(,)?) => {
        eprintln!($fmt_str $(, $arg)*)
    };
}

#[macro_export]
macro_rules! log_err {
    ($fmt_str:expr $(, $arg:expr)* $(,)?) => {
        $crate::log!(concat!("ERROR - ", $fmt_str) $(, $arg)*)
    };
}

#[macro_export]
macro_rules! log_info {
    ($fmt_str:expr $(, $arg:expr)* $(,)?) => {
        $crate::log!(concat!("INFO - ", $fmt_str) $(, $arg)*)
    };
}

