
use rustyline::Editor;
use std::io::Write;
use std::path::Path;
use colored::Colorize;
use std::path::PathBuf;
use std::io::stdout;
use std::io::stdin;
use indicatif::ProgressBar;
use indicatif::ProgressStyle;
use path_absolutize::Absolutize;
use std::fs;
use crate::config::Config;
use crate::call_extern;
use crate::link::string2c;

#[macro_export]
macro_rules! print_error {
    ($x:expr $(, $more:expr)*) => {{
        println!("{}", format!($x, $($more),*).red());
        ::std::process::exit(1);
    }}
}

fn ask_value(prompt: &str, default: &str, required: bool) -> String {
	let text = format!("{}{}: ", prompt, if required { "" } else { " (optional)" });

	let mut rl = Editor::<()>::new();

	loop {
		let readline = rl.readline_with_initial(text.as_str(), (default, ""));
		match readline {
			Ok(line) => {
				rl.add_history_entry(line.as_str());
				if line.is_empty() {
					if required {
						println!("{}", "Please enter a value".red());
					} else {
						return default.to_string();
					}
				} else {
					return line.trim().to_string();
				}
			},
			Err(err) => {
				print_error!("Error: {}", err);
			}
		}
	}
}

pub fn cli_create_template(project_name: Option<String>, location: Option<PathBuf>) {
	let is_location_default = location.is_none();
	let loc = match location {
	    Some(s) => s,
	    None => std::env::current_dir().unwrap()
	};

	let name = match project_name {
		Some(s) => ask_value("Name", s.as_str(), true),
		None => ask_value("Name", "", true)
	};
	let version = ask_value("Version", "v1.0.0", true);
	let developer = ask_value(
		"Developer",
		Config::get().default_developer.as_ref().unwrap_or(&String::new()).as_str(),
		true
	);

	if Config::get().default_developer.is_none() {
		println!("{}{}{}\n{}{}",
			"Using ".bright_cyan(),
			developer,
			" as default developer name for future projects.".bright_cyan(),
			"If this is undesirable, use ".bright_cyan(),
			"`geode config --dev <NAME>`".bright_yellow()
		);
		Config::get().default_developer = Some(developer.clone());
	}

	let description = ask_value("Description", "", false);
	let buffer = if is_location_default {
		loc.absolutize().unwrap().join(&name).to_str().unwrap().to_string()
	} else {
		loc.absolutize().unwrap().to_str().unwrap().to_string()
	};
	let locstr = ask_value("Location", buffer.as_str(), true);
	let project_location = Path::new(&locstr);

	let id = format!(
		"com.{}.{}",
		developer.to_lowercase().replace(' ', "_"),
		name.to_lowercase().replace(' ', "_")
	);
	
	println!(
	    "Creating mod with ID {} named {} by {} version {} in {}",
	    id.green(),
	    name.green(),
	    developer.green(),
	    version.green(),
	    project_location.to_str().unwrap().green()
	);

	if project_location.exists() {
	    println!("{}", "It appears that the provided location already exists.".bright_yellow());
	    print!("{}", "Are you sure you want to proceed? (y/N) ".bright_yellow());
		stdout().flush().unwrap();
		let mut ans = String::new();
		stdin().read_line(&mut ans).unwrap();
		ans = ans.trim().to_string();
		if !(ans == "y" || ans == "Y") {
			println!("{}", "Aborting".bright_red());
			return;
		}
	} else if fs::create_dir_all(&project_location).is_err() {
		print_error!("Unable to create directory for project");
	}

	let bar = ProgressBar::new_spinner();
	bar.enable_steady_tick(120);
	bar.set_style(
		ProgressStyle::default_spinner()
			.tick_strings(&[
				"[##    ]",
				"[###   ]",
				"[####  ]",
				"[ #### ]",
				"[   ###]",
				"[    ##]",
				"[#    #]",
				"[ done ]",
			])
			.template("{spinner:.cyan} {msg}"),
	);
	bar.set_message(format!("{}", "Creating...".bright_cyan()));

	call_extern!(crate::link::geode_create_template(
		string2c(project_location.to_str().unwrap()),
		string2c(name),
		string2c(version),
		string2c(id),
		string2c(developer),
		string2c(description)
	));

	bar.finish_with_message(format!("{}", "Succesfully initialized project! Happy modding :)".bright_cyan()));
}
