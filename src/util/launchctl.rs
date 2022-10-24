use dirs::home_dir;
use std::process::Command;
use std::fs;
use crate::{fail, warn};

fn format_env(path: &str) -> String {
	format!(r#"
		<?xml version="1.0" encoding="UTF-8"?>

		<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">

		<plist version="1.0">
		<dict>
		    <key>Label</key>
		    <string>com.geode-sdk.env</string>
		    <key>ProgramArguments</key>
		    <array>
		    <string>sh</string>
		    <string>-c</string>
		    <string>launchctl setenv GEODE_SDK {}</string>
		    </array>
		    <key>RunAtLoad</key>
		    <true/>
		</dict>
		</plist>
		"#, path)
}

fn start_service(path: &str) -> bool {
	if let Err(e) = Command::new("launchctl")
		.arg("load")
		.arg(path)
		.spawn() {
		fail!("Unable to start launchctl service: {}", e);
		false
	} else {
		true
	}
}

fn restart_service(path: &str) -> bool {
	if let Err(e) = Command::new("launchctl")
		.arg("unload")
		.arg(path)
		.spawn() {

		fail!("Unable to stop launchctl service: {}", e);
		false
	} else {
		start_service(path)
	}
}

pub fn set_sdk_env(path: &str) -> bool {
	let env_dir = home_dir().unwrap().join("Library").join("LaunchAgents").join("com.geode-sdk.env.plist");
	let reinstall = env_dir.exists();

	if let Err(e) = fs::write(&env_dir, format_env(path)) {
		fail!("Unable to write to environments plist: {}", e);
		return false;
	}

	let out = if reinstall {
		restart_service(env_dir.to_str().unwrap())
	} else {
		start_service(env_dir.to_str().unwrap())
	};

	if out {
		warn!("You may have to restart your terminal application for the environment variable changes to go into effect");
	}

	out
}
