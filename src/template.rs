use crate::config::Config;
use crate::sdk::get_version;
use crate::util::config::Template;
use crate::util::logging::{ask_confirm, ask_value};
use crate::{done, fatal, info, warn, NiceUnwrap};
use git2::build::RepoBuilder;
use minijinja::{context, Environment};
use path_absolutize::Absolutize;

use clap::Subcommand;
use serde::Serialize;
use serde_json::json;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;
use walkdir::WalkDir;

#[derive(Subcommand, Debug)]
#[clap(rename_all = "kebab-case")]
pub enum Templates {
	/// Create a new template
	Create {
		/// The name of the template
		name: String,
		/// URL to the template git repository or local path
		repository: String,

		/// The description of the template
		#[arg(short, long)]
		description: Option<String>,
		/// Subfolder to use within the provided repository
		#[arg(short, long)]
		subfolder: Option<String>,
	},

	/// Modify a template
	Modify {
		/// The name of the template to modify
		name: String,
		/// URL to the new template git repository or local path
		#[arg(short, long)]
		repository: Option<String>,
		/// The new description of the template
		#[arg(short, long)]
		description: Option<String>,
		/// Subfolder to use within the provided repository
		#[arg(short, long)]
		subfolder: Option<String>,
	},

	/// List all available templates
	List,

	/// Delete a template
	Delete {
		/// The name of the template to delete
		name: String,
	},
}

struct CreateTemplate<'a> {
	pub template: &'a Template,
	pub project_location: PathBuf,
	pub name: String,
	pub version: String,
	pub id: String,
	pub developer: String,
	pub description: String,
	pub strip: bool,
}

/// This may be either a reference to a local directory or to a subfolder in a cloned repository.
struct TemplateRepo {
	target: PathBuf,
	_temp_dir: Option<TempDir>,
}

fn create_template(template: CreateTemplate) {
	if template.project_location.exists() {
		warn!("The provided location already exists.");
		if !ask_confirm("Are you sure you want to proceed?", false) {
			info!("Aborting");
			return;
		}
	} else {
		fs::create_dir_all(&template.project_location)
			.nice_unwrap("Unable to create project directory");
	}

	let repo = clone_repo(&template);
	for entry in WalkDir::new(&repo.target) {
		let entry = entry.unwrap();
		let rel = entry.path().strip_prefix(&repo.target).unwrap();
		if rel.starts_with(".git") {
			// do not copy .git
			continue;
		}

		let dest = template.project_location.join(rel);
		if entry.file_type().is_dir() {
			fs::create_dir_all(&dest)
				.nice_unwrap(format!("failed to create dir {}", dest.display()));
		} else {
			fs::copy(entry.path(), &dest)
				.nice_unwrap(format!("failed to copy file to {}", dest.display()));
		}
	}

	let mut env = Environment::new();
	let context = context! {
		id => &template.id,
		name => &template.name,
		escaped_name => sanitize_project_name(&template.name),
		geode => &get_version().to_string(),
		version => &template.version,
		developer => &template.developer,
		description => &template.description,
		comments => !template.strip,
	};

	for entry in WalkDir::new(&template.project_location) {
		let entry = entry.unwrap();
		if entry.file_type().is_dir() {
			continue;
		}
		let rel = entry
			.path()
			.strip_prefix(&template.project_location)
			.unwrap();

		// skip some potentially problematic folders
		if rel.starts_with(".git") || rel.starts_with("build") || rel.starts_with(".github") {
			continue;
		}

		// skip non text files
		let allowed_extensions = [
			"txt", "md", "json", "yaml", "yml", "toml", "c", "cpp", "m", "mm", "h", "hpp", "cc",
			"hh", "cxx", "hxx", "py",
		];
		let ext = entry
			.path()
			.extension()
			.map(|s| s.to_string_lossy().to_lowercase())
			.unwrap_or_default();
		if !allowed_extensions.contains(&ext.as_str()) {
			continue;
		}

		let Ok(contents) = fs::read_to_string(entry.path()) else {
			continue;
		};

		if contents.trim().is_empty() {
			continue;
		}

		env.add_template_owned(rel.to_string_lossy().into_owned(), contents)
			.nice_unwrap(format!("failed to add template {}", entry.path().display()));
	}

	for (name, tmpl) in env.templates() {
		let path = template.project_location.join(name);
		let rendered = tmpl
			.render(&context)
			.nice_unwrap(format!("failed to render template {}", path.display()));

		if rendered.trim().is_empty() {
			// if a file is empty, but it was still added as a template, likely the entire file is wrapped in conditional blocks,
			// and they all evaluated as false. so remove the file instead of keeping an empty one
			let _ = fs::remove_file(&path);
		}

		fs::write(&path, rendered).unwrap();
	}

	let mod_json_path = template.project_location.join("mod.json");

	if !mod_json_path.exists() {
		// Default mod.json
		let mod_json = json!({
			"geode":        get_version().to_string(),
			"version":      template.version,
			"id":           template.id,
			"name":         template.name,
			"developer":    template.developer,
			"description":  template.description,
		});

		// Format neatly
		let buf = Vec::new();
		let formatter = serde_json::ser::PrettyFormatter::with_indent(b"\t");
		let mut ser = serde_json::Serializer::with_formatter(buf, formatter);
		mod_json.serialize(&mut ser).unwrap();

		// Write formatted json
		let mod_json_content = String::from_utf8(ser.into_inner()).unwrap();

		fs::write(mod_json_path, mod_json_content)
			.nice_unwrap("Unable to write mod.json, are permissions correct?");
	}
	done!("Succesfully initialized project! Happy modding :)");
}

fn clone_repo(template: &CreateTemplate) -> TemplateRepo {
	let repo = &template.template.repository;
	let subfolder = &template.template.subfolder;

	// is this a remote repo?
	if repo.starts_with("http://")
		|| repo.starts_with("https://")
		|| repo.starts_with("git@")
		|| repo.starts_with("ssh://")
	{
		let temp_dir = TempDir::new().nice_unwrap("Unable to create temporary directory");

		RepoBuilder::new()
			.clone(repo, temp_dir.path())
			.nice_unwrap("Unable to clone repository");

		let mut target = temp_dir.path().to_path_buf();
		if let Some(subfolder) = subfolder {
			target = target.join(subfolder);
		}

		TemplateRepo {
			target,
			_temp_dir: Some(temp_dir),
		}
	} else {
		let mut target = PathBuf::from(repo);
		if let Some(subfolder) = subfolder {
			target = target.join(subfolder);
		}

		if !target.exists() {
			fatal!(
				"The specified template path does not exist: {}",
				target.display()
			);
		}

		TemplateRepo {
			target,
			_temp_dir: None,
		}
	}
}

fn possible_name(path: &Option<PathBuf>) -> Option<String> {
	let path = path.as_ref()?;
	Some(if path.is_absolute() {
		path.file_name()?.to_string_lossy().to_string()
	} else {
		std::env::current_dir()
			.ok()?
			.join(path)
			.file_name()?
			.to_string_lossy()
			.to_string()
	})
}

pub fn build_template(location: Option<PathBuf>) {
	let mut config = Config::new().assert_is_setup();

	info!("This utility will walk you through setting up a new mod.");
	info!("You can change any of the properties you set here later on by editing the generated mod.json file.");

	info!("Choose a template for the mod to be created:");
	info!("Note: you can create your own templates via 'geode template create'");

	let template_index = dialoguer::Select::with_theme(&dialoguer::theme::ColorfulTheme::default())
		.items(config.templates.iter().map(|t| t.describe()))
		.default(0)
		.interact_opt()
		.nice_unwrap("Unable to get template")
		.unwrap_or(0);

	let template = &config.templates[template_index];

	let final_name = ask_value("Name", possible_name(&location).as_deref(), true);

	let location = location.unwrap_or_else(|| std::env::current_dir().unwrap().join(&final_name));
	let location = location.absolutize().unwrap();

	let final_version = ask_value("Version", Some("v1.0.0"), true);

	let final_developer = ask_value("Developer", config.default_developer.as_deref(), true);

	if config.default_developer.is_none() {
		info!(
			"Using '{}' as the default developer for all future projects. \
			If this is undesirable, you can set a default developer using \
			`geode config set default-developer <name>`",
			&final_developer
		);
		config.default_developer = Some(final_developer.clone());
		config.save();
	}

	let final_description = ask_value("Description", None, false);
	let final_location = PathBuf::from(ask_value(
		"Location",
		Some(&location.to_string_lossy()),
		true,
	));

	let mod_id = format!(
		"{}.{}",
		final_developer
			.to_lowercase()
			.replace(' ', "-")
			.replace("\"", ""),
		final_name
			.to_lowercase()
			.replace(' ', "-")
			.replace("\"", "")
	);

	let strip = ask_confirm(
		"Do you want to remove comments from the default template?",
		false,
	);

	info!("Creating project {}", mod_id);
	create_template(CreateTemplate {
		template,
		project_location: final_location,
		name: final_name.replace("\"", "\\\""),
		version: final_version,
		id: mod_id,
		developer: final_developer.replace("\"", "\\\""),
		description: final_description.replace("\"", "\\\""),
		strip,
	});
}

fn sanitize_project_name(mut name: &str) -> String {
	name = name.trim_matches(|c: char| !c.is_ascii_alphanumeric());

	if name.is_empty() {
		return "mod".to_string();
	}

	name.chars()
		.map(|c| match c {
			'A'..='Z' | 'a'..='z' | '0'..='9' | '_' | '-' | '.' => c,
			_ => '_',
		})
		.collect()
}

fn create_new_template(config: &mut Config, template: Template) {
	if config.templates.iter().any(|t| t.name == template.name) {
		fatal!(
			"A template with the name '{}' already exists",
			template.name
		);
	}

	config.templates.push(template);
}

fn modify_template(
	config: &mut Config,
	name: String,
	description: Option<String>,
	repository: Option<String>,
	subfolder: Option<String>,
) {
	let template = config
		.templates
		.iter_mut()
		.find(|t| t.name == name)
		.unwrap_or_else(|| fatal!("No template with the name '{}' exists", name));

	if let Some(desc) = description {
		template.description = (!desc.is_empty()).then_some(desc);
	}
	if let Some(repo) = repository {
		template.repository = repo;
	}
	if let Some(subfolder) = subfolder {
		template.subfolder = (!subfolder.is_empty()).then_some(subfolder);
	}
}

fn list_templates(config: &Config) {
	for (i, template) in config.templates.iter().enumerate() {
		info!("{}. {}", i + 1, template.describe());
		info!(" - Repository: {}", template.repository);
		if let Some(subfolder) = &template.subfolder {
			info!(" - Subfolder: {}", subfolder);
		}
	}
}

fn delete_template(config: &mut Config, name: String) {
	config.templates.retain(|t| t.name != name);
}

pub fn subcommand(cmd: Templates) {
	let mut config = Config::new();

	match cmd {
		Templates::Create {
			name,
			description,
			repository,
			subfolder,
		} => {
			create_new_template(
				&mut config,
				Template {
					name,
					description,
					repository,
					subfolder,
				},
			);
		}
		Templates::Modify {
			name,
			description,
			repository,
			subfolder,
		} => {
			modify_template(&mut config, name, description, repository, subfolder);
		}

		Templates::List => {
			list_templates(&config);
		}

		Templates::Delete { name } => {
			delete_template(&mut config, name);
		}
	}

	config.save();
}
