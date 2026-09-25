use crate::config::Config;
use crate::server::ApiResponse;
use crate::util::logging::{ask_confirm, ask_value};
use crate::{NiceUnwrap, info, done, fail, fatal, index};
use reqwest::header::USER_AGENT;
use serde::Deserialize;
use serde_json::json;
use std::fmt::Display;

#[derive(Deserialize)]
pub struct ModDeprecation {
    pub id: i32,
    #[allow(unused)]
    pub mod_id: String,
    pub by: Vec<String>,
    reason: String
}

impl Display for ModDeprecation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "- ID: {}", self.id)?;
        writeln!(f, "- Reason: {}", self.reason)?;
        if !self.by.is_empty() {
            writeln!(f, "- Alternatives:")?;
            for (i, alt) in self.by.iter().enumerate() {
                writeln!(f, "   {}. {}", i + 1, alt)?;
            }
        }
        Ok(())
    }
}

fn get_mod_deprecations(id: &str, config: &Config) -> Vec<ModDeprecation> {
    if config.index_token.is_none() {
        fatal!("You are not logged in");
    }

    let client = reqwest::blocking::Client::new();
    let url = index::get_index_url(format!("/v1/mods/{}/deprecations", id), config);

    let response = client
        .get(url)
        .header(USER_AGENT, "GeodeCLI")
        .send()
        .nice_unwrap("Unable to connect to Geode Index");

    if response.status() == 404 {
        fatal!("Mod '{}' doesn't exist", id);
    }

    if !response.status().is_success() {
        let body: ApiResponse<String> = response
            .json()
            .nice_unwrap("Unable to get deprecations from mod");
        fatal!("Unable to get deprecations from mod: {}", body.error);
    }

    let body: ApiResponse<Vec<ModDeprecation>> = response
        .json()
        .nice_unwrap("Unable to parse response from Geode Index");
    body.payload
}

fn get_mod_alternatives(config: &Config) -> Vec<String> {
    let mut ret = vec![];
    let confirm = ask_confirm("Do you want to add any mod alternatives?", true);
    if confirm {
        loop {
            let alternative = ask_value("Mod alternative ID (leave empty to finish)", None, false);
            if alternative.is_empty() {
                break;
            }

            // Making sure the mod exists
            let response = reqwest::blocking::get(index::get_index_url(format!("/v1/mods/{}", alternative), config))
                .nice_unwrap("Unable to connect to Geode Index");
            if response.status() == 404 {
                fail!("Mod {} doesn't exist", alternative);
            } else {
                info!("Found mod '{}'", alternative);
                ret.push(alternative);
            }
        }
    }
    ret
}

pub fn add_deprecation(id: Option<String>, reason: Option<String>, config: &Config) {
    if config.index_token.is_none() {
        fatal!("You are not logged in");
    }

    let id = id.unwrap_or_else(|| ask_value("Mod ID", None, true));
    let reason = reason.unwrap_or_else(|| ask_value("Reason", None, true));
    let by = get_mod_alternatives(config);

    let confirm = ask_confirm(&format!("Are you sure you want to deprecate '{}'?", &id), false);

    if !confirm {
        done!("Operation cancelled");
        return
    }

    let client = reqwest::blocking::Client::new();
    let url = index::get_index_url(format!("/v1/mods/{}/deprecations", id), config);

    info!("Deprecating mod");

    let response = client
        .post(url)
        .header(USER_AGENT, "GeodeCLI")
        .bearer_auth(config.index_token.clone().unwrap())
        .json(&json!({ "by": by, "reason": reason }))
        .send()
        .nice_unwrap("Unable to connect to Geode Index");

    if response.status() == 404 {
        fatal!("Mod '{}' doesn't exist", id);
    }

    if !response.status().is_success() {
        let body: ApiResponse<String> = response
            .json()
            .nice_unwrap("Unable to deprecate mod");
        fatal!("Unable to deprecate mod: {}", body.error);
    }

    done!("Mod deprecated successfully");
}

pub fn remove_deprecation(id: Option<String>, config: &Config) {
    if config.index_token.is_none() {
        fatal!("You are not logged in");
    }

    let id = id.unwrap_or_else(|| ask_value("Mod ID", None, true));

    let client = reqwest::blocking::Client::new();
    let url = index::get_index_url(format!("/v1/mods/{}/deprecations", id), config);

    info!("Removing all deprecations from mod {}", id);

    let response = client
        .delete(url)
        .header(USER_AGENT, "GeodeCLI")
        .bearer_auth(config.index_token.clone().unwrap())
        .send()
        .nice_unwrap("Unable to connect to Geode Index");

    if response.status() == 404 {
        fatal!("Mod '{}' doesn't exist", id);
    }

    if !response.status().is_success() {
        let body: ApiResponse<String> = response
            .json()
            .nice_unwrap("Unable to remove deprecations from mod");
        fatal!("Unable to remove deprecations from mod: {}", body.error);
    }

    info!("Removed all deprecations from mod '{}'", id);
}

pub fn print_mod_deprecations(id: Option<String>, config: &Config) {
    let id = id.unwrap_or_else(|| ask_value("Mod ID", None, true));

    let deprecations = get_mod_deprecations(&id, config);
    if deprecations.is_empty() {
        fatal!("Mod '{}' has no deprecations", id);
    }

    info!("Deprecations:");
    for (i, deprecation) in deprecations.iter().enumerate() {
        println!("{}).", i + 1);
        println!("{}", deprecation);
    }
}

pub fn update_deprecation(
    mod_id: Option<String>,
    deprecation_id: Option<String>,
    reason: Option<String>,
    config: &Config
) {
    if config.index_token.is_none() {
        fatal!("You are not logged in");
    }

    let mod_id = mod_id.unwrap_or_else(|| ask_value("Mod ID", None, true));
    let deprecation_id = deprecation_id.unwrap_or_else(|| ask_value("Deprecation ID", None, true));
    let reason = reason.unwrap_or_else(|| ask_value("Reason", None, true));
    let by = get_mod_alternatives(config);

    let client = reqwest::blocking::Client::new();
    let url = index::get_index_url(format!("/v1/mods/{}/deprecations/{}", mod_id, deprecation_id), config);

    info!("Updating deprecation");

    let deprecations = get_mod_deprecations(&mod_id, config);
    if deprecations.is_empty() {
        fatal!("Mod '{}' has no deprecations", mod_id);
    }

    let response = client
        .put(url)
        .header(USER_AGENT, "GeodeCLI")
        .bearer_auth(config.index_token.clone().unwrap())
        .json(&json!({ "by": by, "reason": reason }))
        .send()
        .nice_unwrap("Unable to connect to Geode Index");

    if response.status() == 404 {
        fatal!("Deprecation '{}' doesn't exist", deprecation_id);
    }

    if !response.status().is_success() {
        let body: ApiResponse<String> = response
            .json()
            .nice_unwrap("Unable to update deprecation");
        fatal!("Unable to update deprecation: {}", body.error);
    }

    done!("Updated deprecation successfully");
}
