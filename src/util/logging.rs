use std::fmt::Display;

#[macro_export]
macro_rules! info {
    ($x:expr $(, $more:expr)*) => {{
    	use ::colored::Colorize;
        println!("{}{}", "| Info | ".bright_cyan(), format!($x, $($more),*));
    }}
}

#[macro_export]
macro_rules! fail {
    ($x:expr $(, $more:expr)*) => {{
    	use ::colored::Colorize;
        println!("{}{}", "| Fail | ".bright_red(), format!($x, $($more),*));
    }}
}

#[macro_export]
macro_rules! fatal {
    ($x:expr $(, $more:expr)*) => {{
        use ::colored::Colorize;
        println!("{}{}", "| Fail | ".bright_red(), format!($x, $($more),*));
        std::process::exit(1);
    }}
}

#[macro_export]
macro_rules! warn {
    ($x:expr $(, $more:expr)*) => {{
    	use ::colored::Colorize;
        println!("{}{}", "| Warn | ".bright_yellow(), format!($x, $($more),*));
    }}
}

#[macro_export]
macro_rules! done {
    ($x:expr $(, $more:expr)*) => {{
    	use ::colored::Colorize;
        println!("{}{}", "| Done | ".bright_green(), format!($x, $($more),*));
    }}
}


pub trait NiceUnwrap<T> {
    fn nice_unwrap<S: Display>(self, text: S) -> T;
}

impl<T, E: Display> NiceUnwrap<T> for Result<T, E> {
    fn nice_unwrap<S: Display>(self, text: S) -> T {
        self.unwrap_or_else(|e| fatal!("{}: {}", text, e))
    }
}

impl<T> NiceUnwrap<T> for Option<T> {
    fn nice_unwrap<S: Display>(self, text: S) -> T {
        self.unwrap_or_else(|| fatal!("{}", text))
    }
}
