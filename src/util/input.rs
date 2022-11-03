use crate::{fail, NiceUnwrap};
use rustyline::Editor;

pub fn ask_value(prompt: &str, default: Option<&str>, required: bool) -> String {
	let text = format!("{}{}: ", prompt, if required { "" } else { " (optional)" });
	let mut line_reader = Editor::<()>::new();
	loop {
		let line = line_reader
			.readline_with_initial(&text, (default.unwrap_or(""), ""))
			.nice_unwrap("Error reading line");
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
