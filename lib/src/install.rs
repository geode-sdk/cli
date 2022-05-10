use git2::{FetchOptions, Repository, RemoteCallbacks, SubmoduleUpdateOptions, Progress, build::RepoBuilder};

use std::io::{Result, Error, ErrorKind};
use std::path::Path;
use std::os::raw::c_char;

use crate::string2c;

pub type SuiteProgressCallback = extern "stdcall" fn(*const c_char, i32) -> ();

pub fn install_suite(
    path: &Path,
    nightly: bool,
	callback: SuiteProgressCallback
) -> Result<()> {
    let prog_fn = |info: &String, prog: Progress| {
        let percentage =
            if prog.total_objects() > 0 {
                prog.received_objects() as f32 / prog.total_objects() as f32 * 100f32
            } else { 0f32 };
        unsafe {
            callback(string2c(format!(
                "{}; Objects: {} / {}, deltas: {} / {}, bytes: {:.2}mb",
                info,
                prog.received_objects(),
                prog.total_objects(),
                prog.indexed_deltas(),
                prog.total_deltas(),
                prog.received_bytes() as f32 / 1000000.0
            )), percentage as i32);
        }
        true
    };

    let mut callbacks = RemoteCallbacks::new();
    callbacks.transfer_progress(|prog: Progress| {
        prog_fn(&String::from("Cloning suite"), prog)
    });

    let mut opts = FetchOptions::new();
    opts.remote_callbacks(callbacks);

    let mut builder = RepoBuilder::new();
    builder.fetch_options(opts);
    if nightly {
        builder.branch("nightly");
    }

    match builder.clone(
        "https://github.com/geode-sdk/suite",
        path
    ) {
        Ok(repo) => {
            // due to reasons i'm not quite sure of 
            // Repository::update_submodules is private
            let add_subrepos = |repo: &Repository, list: &mut Vec<Repository>| -> std::result::Result<(), git2::Error> {
                for mut subm in repo.submodules()? {
                    let current_sub = match subm.name() {
                        Some(s) => String::from(s),
                        None => String::from("Unknown")
                    };

                    let mut callbacks = RemoteCallbacks::new();
                    callbacks.transfer_progress(|prog: Progress| {
                        prog_fn(&(String::from("Cloning submodule ") + &current_sub), prog)
                    });

                    let mut opts = FetchOptions::new();
                    opts.remote_callbacks(callbacks);

                    let mut sopts = SubmoduleUpdateOptions::new();
                    sopts.fetch(opts);

                    subm.update(true, Some(&mut sopts))?;
                    list.push(subm.open()?);
                }
                Ok(())
            };
    
            let mut repos = Vec::new();
            add_subrepos(&repo, &mut repos).unwrap();
            while let Some(repo) = repos.pop() {
                add_subrepos(&repo, &mut repos).unwrap();
            }
            Ok(())
        },
        Err(e) => Err(Error::new(
            ErrorKind::Other,
            format!("Error cloning repository: {}", e)
        ))
    }
}
