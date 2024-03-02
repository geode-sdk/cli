use std::fmt::Display;
use std::io::Write;

use rustyline::Editor;

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
        eprintln!("{}{}", "| Fail | ".bright_red(), format!($x, $($more),*));
    }}
}

#[macro_export]
macro_rules! fatal {
    ($x:expr $(, $more:expr)*) => {{
        use ::colored::Colorize;
        eprintln!("{}{}", "| Fail | ".bright_red(), format!($x, $($more),*));
        std::process::exit(1);
    }}
}

#[macro_export]
macro_rules! warn {
    ($x:expr $(, $more:expr)*) => {{
    	use ::colored::Colorize;
        eprintln!("{}{}", "| Warn | ".bright_yellow(), format!($x, $($more),*));
    }}
}

#[macro_export]
macro_rules! done {
    ($x:expr $(, $more:expr)*) => {{
    	use ::colored::Colorize;
        println!("{}{}", "| Done | ".bright_green(), format!($x, $($more),*));
    }}
}

#[macro_export]
macro_rules! confirm {
    ($x:expr $(, $more:expr)*) => {
        $crate::logging::ask_confirm(&format!($x, $($more),*), false)
    };
}

pub fn clear_terminal() {
	print!("{esc}c", esc = 27 as char);
}

pub fn ask_value(prompt: &str, default: Option<&str>, required: bool) -> String {
	let text = format!("{}{}: ", prompt, if required { "" } else { " (optional)" });
	let mut line_reader = Editor::<(), rustyline::history::DefaultHistory>::new().unwrap();
	loop {
		let line = line_reader
			.readline_with_initial(&text, (default.unwrap_or(""), ""))
			.expect("Error reading line");
		line_reader
			.add_history_entry(&line)
			.expect("Error reading line");

		if line.is_empty() {
			if required {
				fail!("Please enter a value");
			} else {
				return default.unwrap_or("").to_string();
			}
		} else {
			return line.trim().to_string();
		}
	}
}

pub fn ask_confirm(text: &str, default: bool) -> bool {
	use colored::Colorize;
	// print question
	print!(
		"{}{} {} ",
		"| Okay | ".bright_purple(),
		text,
		if default { "(Y/n)" } else { "(y/N)" }
	);
	std::io::stdout().flush().unwrap();
	let mut yes = String::new();
	match std::io::stdin().read_line(&mut yes) {
		Ok(_) => match yes.trim().to_lowercase().as_str() {
			"yes" | "ye" | "y" => true,
			"no" | "n" => false,
			_ => default,
		},
		Err(_) => default,
	}
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
