use git2::Repository;

use std::io::{Result, Error, ErrorKind};
use std::path::Path;

pub fn install_suite(path: &Path) -> Result<()> {
    return match Repository::clone_recurse(
        "https://github.com/geode-sdk/suite",
        path
    ) {
        Ok(_) => Ok(()),
        Err(e) => Err(Error::new(
            ErrorKind::Other,
            format!("Error cloning repository: {}", e)
        ))
    };
}
