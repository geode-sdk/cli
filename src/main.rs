use std::path::PathBuf;
use clap::Args;
use colored::*;
use clap::Parser;
use clap::Subcommand;
use path_absolutize::*;

#[derive(Parser)]
#[clap(version, long_about = None)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Adds files to myapp
    New {
        location: PathBuf,
        name: Option<String>
    },
}
fn main() {
    let args = Cli::parse();
    match args.command {
        Commands::New { location, name } => {
            let absolute_location = location.absolutize().unwrap();
            let project_name = match name {
                Some(s) => s,
                None => absolute_location.file_name().unwrap().to_str().unwrap().to_string()
            };
            println!("insane {} name {}", absolute_location.to_str().unwrap().blue(), project_name);
        }
    }
}