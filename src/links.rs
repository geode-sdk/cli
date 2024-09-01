use clap::Subcommand;
use webbrowser;
use std::io::{self, Write};
use crate::{info, fail};

#[derive(Subcommand, Debug)]
pub enum Links {
    Show,
}

pub fn subcommand(cmd: Links) {
    match cmd {
        Links::Show => {
            info!("Select a link to open:");
            info!("1. Geode Website");
            info!("2. Geode Repository");
            info!("3. Geode Issues");
            info!("4. Geode Discord");

            print!("Enter a number: ");
            io::stdout().flush().unwrap();

            let mut input = String::new();
            io::stdin().read_line(&mut input).expect("Failed to read line");
            let choice = input.trim();

            match choice {
                "1" => {
                    info!("Opening Geode Website...");
                    if let Err(e) = webbrowser::open("https://geode-sdk.org/") {
                        fail!("Failed to open link: {}", e);
                    }
                }
                "2" => {
                    info!("Opening Geode Repository...");
                    if let Err(e) = webbrowser::open("https://github.com/geode-sdk/geode") {
                        fail!("Failed to open link: {}", e);
                    }
                }
                "3" => {
                    info!("Opening Geode Issues...");
                    if let Err(e) = webbrowser::open("https://github.com/geode-sdk/geode/issues") {
                        fail!("Failed to open link: {}", e);
                    }
                }
                "4" => {
                    info!("Opening Geode Discord...");
                    if let Err(e) = webbrowser::open("https://discord.gg/9e43WMKzhp") {
                        fail!("Failed to open link: {}", e);
                    }
                }
                _ => {
                    fail!("Invalid choice. Please enter a number between 1 and 4.");
                }
            }
        }
    }
}
