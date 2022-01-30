use serde::*;
use std::fs;
use std::env;
use std::io;
use std::path::*;

#[derive(Serialize, Deserialize)]
struct ModInfo 
{
    mod_name: String,
    location: String,
}

#[derive(Serialize, Deserialize)]
struct Mods 
{
    mods: Vec<ModInfo>,
}

pub fn exists(path: &str) -> bool {
    fs::metadata(path).is_ok()
}

pub fn add_new_project_to_list(name: String, location: String)
{
	if !exists(get_list_location().as_str()) { create_project_list(); }

	let mod_str = fs::read_to_string(get_list_location()).expect("Oops!");
	let final_location = format!("{}/{}", location, name);

	let mut m : Mods = serde_json::from_str(&mod_str).unwrap();
	let new_mod = ModInfo { mod_name: name, location: final_location };
	m.mods.push(new_mod);

	fs::write(get_list_location(), serde_json::to_string(&m).unwrap()).expect("Unable to rewrite list");
}

pub fn get_list_location() -> String
{
	return format!("{}/list.json", get_exe_directory().unwrap().into_os_string().into_string().unwrap());
}

pub fn get_exe_directory() -> io::Result<PathBuf> {
    let mut dir = env::current_exe()?;
    dir.pop();
    Ok(dir)
}

pub fn create_project_list()
{
	let data = r#"{
		"mods": []
	}"#;
	fs::write(get_list_location(), data).expect("Unable to create list");
}

pub fn get_project_info(name: String) -> String
{
	let mut count = 0usize;

	let mod_str = fs::read_to_string(get_list_location()).expect("Oops!");
	let m : Mods = serde_json::from_str(&mod_str).unwrap();

	loop 
	{
		if count > m.mods.len() - 1
		{
			return "".to_string();
		}

		if m.mods[count].mod_name == name
		{
			let final_str = &m.mods[count];
			return final_str.location.clone();
		}

		count += 1;
	}
}