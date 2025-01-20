#[cfg(not(target_os = "android"))]
use cli_clipboard::ClipboardProvider;
use reqwest::header::USER_AGENT;
use serde::Deserialize;
use serde_json::json;

use crate::{
	config::Config, done, fatal, index, info, logging::ask_value, server::ApiResponse, warn,
	NiceUnwrap,
};

#[derive(Debug, Deserialize)]
struct LoginAttempt {
	uuid: String,
	interval: i32,
	uri: String,
	code: String,
}

#[cfg(not(target_os = "android"))]
pub fn copy_token(token: &str) {
	if let Ok(mut ctx) = cli_clipboard::ClipboardContext::new() {
		if ctx.set_contents(token.to_string()).is_ok() {
			done!("Token has been copied to your clipboard");
		}
	}
}

#[cfg(target_os = "android")]
pub fn copy_token(token: &str) {
	if terminal_clipboard::set_string(token).is_ok() {
		done!("Token has been copied to your clipboard");
	}
}

pub fn login(config: &mut Config, token: Option<String>, github_token: Option<String>) {
	if let Some(token) = token {
		config.index_token = Some(token);
		config.save();
		done!("Successfully logged in with the provided token");
		return;
	}

	if config.index_token.is_some() {
		warn!("You are already logged in");
		let token = config.index_token.clone().unwrap();
		info!("Your token is: {}", token);
		return;
	}

	let client = reqwest::blocking::Client::new();

	if let Some(github_token) = github_token {
		let response = client
			.post(index::get_index_url("/v1/login/github/token", config))
			.header(USER_AGENT, "GeodeCli")
			.json(&json!({ "token": github_token }))
			.send()
			.nice_unwrap("Unable to connect to Geode Index");

		let parsed: ApiResponse<String> = match response.status().as_u16() {
			400 => fatal!("Invalid Github Token"),
			200 => response.json().nice_unwrap("Unable to parse login response"),
			_ => fatal!("Unable to connect to Geode Index"),
		};

		config.index_token = Some(parsed.payload);
		config.save();
		done!("Successfully logged in via Github token");
		return;
	}

	let response: reqwest::blocking::Response = client
		.post(index::get_index_url("/v1/login/github", config))
		.header(USER_AGENT, "GeodeCli")
		.json(&{})
		.send()
		.nice_unwrap("Unable to connect to Geode Index");

	if response.status() != 200 {
		fatal!("Unable to connect to Geode Index");
	}

	let parsed = response
		.json::<ApiResponse<LoginAttempt>>()
		.nice_unwrap("Unable to parse login response");

	let login_data = parsed.payload;

	info!("You will need to complete the login process in your web browser");
	info!("Go to: {} and enter the login code", &login_data.uri);
	info!("Your login code is: {}", &login_data.code);
	copy_token(&login_data.code);
	if let Err(msg) = open::that(&login_data.uri) {
		warn!("Unable to open browser: {}", msg);
		warn!("Go to the URL manually: {}", &login_data.uri);
	}

	loop {
		info!("Checking login status");
		if let Some(token) = poll_login(&client, &login_data.uuid, config) {
			config.index_token = Some(token);
			config.save();
			done!("Login successful");
			break;
		}

		std::thread::sleep(std::time::Duration::from_secs(login_data.interval as u64));
	}
}

fn poll_login(
	client: &reqwest::blocking::Client,
	uuid: &str,
	config: &mut Config,
) -> Option<String> {
	let response = client
		.post(index::get_index_url(
			"/v1/login/github/poll",
			config,
		))
		.json(&json!({ "uuid": uuid }))
		.header(USER_AGENT, "GeodeCLI")
		.send()
		.nice_unwrap("Unable to connect to Geode Index");

	if response.status() != 200 {
		return None;
	}

	let parsed = response
		.json::<ApiResponse<String>>()
		.nice_unwrap("Unable to parse login response");

	Some(parsed.payload)
}

pub fn invalidate(config: &mut Config) {
	if config.index_token.is_none() {
		warn!("You are not logged in");
		return;
	}
	loop {
		let response = ask_value(
			"Do you want to log out of all devices (y/n)",
			Some("n"),
			true,
		);

		match response.to_lowercase().as_str() {
			"y" => {
				invalidate_index_tokens(config);
				config.index_token = None;
				config.save();
				done!("All tokens for the current account have been invalidated successfully");
				break;
			}
			"n" => {
				done!("Operation cancelled");
				break;
			}
			_ => {
				warn!("Invalid response");
			}
		}
	}
}

fn invalidate_index_tokens(config: &mut Config) {
	if config.index_token.is_none() {
		fatal!("You are not logged in");
	}

	let token = config.index_token.clone().unwrap();

	let client = reqwest::blocking::Client::new();

	let response = client
		.delete(index::get_index_url("/v1/me/tokens", config))
		.header(USER_AGENT, "GeodeCLI")
		.bearer_auth(token)
		.send()
		.nice_unwrap("Unable to connect to Geode Index");

	if response.status() == 401 {
		config.index_token = None;
		config.save();
		fatal!("Invalid token. Please login again.");
	}
	if response.status() != 204 {
		fatal!("Unable to invalidate token");
	}
}
