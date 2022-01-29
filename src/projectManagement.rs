use serde::*;
use serde_json::*;
use std::fs;
use std::env;
use std::io;
use std::path::*;

#[derive(Serialize, Deserialize)]
struct ModInfo 
{
    modName: String,
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

pub fn add_new_project_to_list(nameP: String, locationP: String)
{
	if !exists(get_list_location().as_str()) { create_project_list(); }

	let modString = fs::read_to_string(get_list_location()).expect("Oops!");
	let finalLocation = format!("{}/{}", locationP, nameP);

	let mut m : Mods = serde_json::from_str(&modString).unwrap();
	let newMod = ModInfo { modName: nameP, location: finalLocation };
	m.mods.push(newMod);

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

	let modString = fs::read_to_string(get_list_location()).expect("Oops!");
	let mut m : Mods = serde_json::from_str(&modString).unwrap();

	loop 
	{
		if count > m.mods.len() - 1
		{
			return "".to_string();
		}

		if m.mods[count].modName == name
		{
			let finalStr = &m.mods[count];
			return finalStr.location.clone();
		}

		count += 1;
	}
}