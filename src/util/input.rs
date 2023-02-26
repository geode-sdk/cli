use std::io::{stdout, stdin, Write};

use crate::fail;
use rustyline::Editor;

pub fn ask_value(prompt: &str, default: Option<&str>, required: bool) -> String {
	let text = format!("{}{}: ", prompt, if required { "" } else { " (optional)" });
	let mut line_reader = Editor::<()>::new();
	loop {
		let line = line_reader
			.readline_with_initial(&text, (default.unwrap_or(""), ""))
			.expect("Error reading line");
		line_reader.add_history_entry(&line);

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

pub fn ask_yesno(prompt: &str, default: bool) -> bool {
	print!("{} ({}) ", prompt, if default { "Y/n" } else { "y/N" });

	stdout().flush().unwrap();

	let mut ans = String::new();
	stdin().read_line(&mut ans).unwrap();
	ans = ans.trim().to_string().to_lowercase();
	match ans.as_str() {
		"y" | "ye" | "yes" => true,
		"n" | "no" => false,
		_ => default
	}
}
