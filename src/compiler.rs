use crate::project_management;
use std::process::Command;
use std::process::Stdio;
use std::fs;
use std::env;

pub fn run_project(name: String, ide: String)
{
	let mut location = project_management::get_project_info(name.clone());

	if location == "" { println!("{} not found in Geode's list.", name); return; }

	println!("MSBuild Result: {}", windows_find_visual_studio_ms_build());
}

pub fn windows_find_visual_studio_ms_build() -> String
{
	if cfg!(windows)
	{
		// "Ah yes, let's make a long ass fucking command to just get the compiler,
		// what a great idea" -Some dumbass Microsoft executive or chief programmer.

		// This is taking me 3 hours, why...

		// Update: Since this is taking me more than 5 fucking hours.

		// Update 2: It's taking me 7 hours because of Rust's library not taking string literals, fuck this, i'm using a .bat file.

		// Update 3: Realized mid-way that its stupid to have a .bat file, so i'll make it do a .bat file, run it, then delete it afterwards.

		// Update 4: Finally, 8 Hours trying to get this shit running, fuck you Microsoft.
		let mut bat_url_final = win_vs_msb_create_bat_file();

		let command = Command::new(&bat_url_final)
		.stdout(Stdio::piped())
		.spawn()
		.expect("Uh oh, looks like Misocroft's compiler wasn't happy enough, probably the motherfucker needs even more arguments.")
		.wait();

		let finalUrl = win_vs_mbs_get_from_txt();
		win_vs_msb_delete_bat_file();

		return finalUrl;
	}
	else {
		return "".to_string();
	}
}

pub fn win_vs_msb_create_bat_file() -> String
{
	let bat_string = r#""%ProgramFiles(x86)%\Microsoft Visual Studio\Installer\vswhere.exe" -latest -prerelease -products * -requires Microsoft.Component.MSBuild -find MSBuild\**\Bin\MSBuild.exe >> msbuild_output.txt"#;
	let mut bat_url = format!("{}/msbuild_comp.bat", env::current_dir().unwrap().into_os_string().into_string().unwrap());

	fs::write(&bat_url, bat_string).expect(".bat file not created");
	return bat_url;
}

pub fn win_vs_mbs_get_from_txt() -> String
{
	let mut txt_url = format!("{}/msbuild_output.txt", env::current_dir().unwrap().into_os_string().into_string().unwrap());
	return fs::read_to_string(txt_url).expect("Oops!");
}

pub fn win_vs_msb_delete_bat_file()
{
	let bat_url = format!("{}/msbuild_comp.bat", env::current_dir().unwrap().into_os_string().into_string().unwrap());
	fs::remove_file(bat_url);

	let txt_url = format!("{}/msbuild_output.txt", env::current_dir().unwrap().into_os_string().into_string().unwrap());
	fs::remove_file(txt_url);
}