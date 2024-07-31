mod cli;
mod file;
mod index;
mod index_admin;
mod index_auth;
mod index_dev;
mod info;
mod package;
mod profile;
mod project;
mod project_build;
mod sdk;
mod server;
mod template;
mod util;

use crate::profile::RunBackground;
use clap::{CommandFactory, Parser};
use cli::{Args, GeodeCommands};
use util::*;

fn main() {
    #[cfg(windows)]
    match ansi_term::enable_ansi_support() {
        Ok(_) => {}
        Err(_) => println!("Unable to enable color support, output may look weird!"),
    };

    let args = Args::parse();

    let mut config = config::Config::new();

    match args.command {
        GeodeCommands::New { path, api } => template::build_template(&mut config, path, api),
        GeodeCommands::Profile { commands } => profile::subcommand(&mut config, commands),
        GeodeCommands::Config { commands } => info::subcommand(&mut config, commands),
        GeodeCommands::Sdk { commands } => sdk::subcommand(&mut config, commands),
        GeodeCommands::Package { commands } => package::subcommand(&mut config, commands),
        GeodeCommands::Project { commands } => project::subcommand(&mut config, commands),
        GeodeCommands::Index { commands } => index::subcommand(&mut config, commands),
        GeodeCommands::Run {
            background,
            stay,
            launch_args,
        } => profile::run_profile(
            &config,
            None,
            match (background, stay) {
                (false, false) => RunBackground::Foreground,
                (false, true) => RunBackground::ForegroundStay,
                (true, false) => RunBackground::Background,
                (true, true) => panic!("Impossible argument combination (background and stay)"),
            },
            launch_args,
        ),
        GeodeCommands::Build {
            platform,
            configure_only,
            build_only,
            ndk,
            config,
            extra_conf_args,
        } => project_build::build_project(
            platform,
            configure_only,
            build_only,
            ndk,
            config,
            extra_conf_args,
        ),
        GeodeCommands::Completions { shell } => {
            let mut app = Args::command();
            let bin_name = app.get_name().to_string();
            clap_complete::generate(shell, &mut app, bin_name, &mut std::io::stdout());
        }
        GeodeCommands::GenerateManpage {} => {
            let app = Args::command();
            let man = clap_mangen::Man::new(app);
            let _ = man.render(&mut std::io::stdout());
        }
    }

    config.save();
}
