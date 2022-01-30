use crate::project_management;
use std::process::Command;
use std::process::Stdio;
use std::fs;
use std::env;

pub fn run_project(name: String, ide: String, gameLoc: String)
{
	let mut location = project_management::get_project_info(name.clone());

	if location == "" { println!("{} not found in Geode's list.", name); return; }

	if ide == "visualstudio"
	{
		let projectSlnFinal = format!("{}/build/{}.sln", location, name);
		win_vs_msb_build(projectSlnFinal);

		if path_exists(format!("{}/build/Release/{}.dll", location, name).as_str())
		{
			// TODO: 
			// - If there's not a .geode file, make it.
			// - If there's a .geode file/Once there's a .geode file, put it in the geode/mods directory.
		}
		let id = 322170;
		let cmd = format!("steam://rungameid/{}", id.to_string());
		Command::new("cmd").arg("/c").arg("start").arg(cmd).spawn().expect("Uh oh!");
	}
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
	let txt = fs::read_to_string(txt_url).expect("Oops!");
	let finalTxt = txt.replace("\n", "");
	return finalTxt;
}

pub fn win_vs_msb_delete_bat_file()
{
	let bat_url = format!("{}/msbuild_comp.bat", env::current_dir().unwrap().into_os_string().into_string().unwrap());
	fs::remove_file(bat_url);

	let txt_url = format!("{}/msbuild_output.txt", env::current_dir().unwrap().into_os_string().into_string().unwrap());
	fs::remove_file(txt_url);
}

pub fn path_exists(path: &str) -> bool {
    fs::metadata(path).is_ok()
}

pub fn win_vs_msb_build(project_sln: String)
{
	let bat_string = format!("\"{}\" \"{}\" /p:Configuration=\"Release\" /p:Platform=\"Win32\"", windows_find_visual_studio_ms_build(), project_sln);
	let mut bat_url = format!("{}/msbuild_project_comp.bat", env::current_dir().unwrap().into_os_string().into_string().unwrap());

	fs::write(&bat_url, bat_string).expect(".bat file not created");

	let command = Command::new(&bat_url)
		.spawn()
		.expect("Uh oh, looks like Misocroft's compiler wasn't happy enough, probably the motherfucker needs even more arguments.")
		.wait();

	fs::remove_file(bat_url);
}