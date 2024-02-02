use std::process::Command;

use crate::{fail, fatal, info, NiceUnwrap};

pub fn build_project(platform: Option<String>, _extra_conf_args: Option<String>) {
	let mut cross_compiling = false;
	let platform = platform.map(|x| x.to_lowercase());
	let platform = platform
		.map(|x| match x.as_str() {
			"win" | "windows" => String::from("win"),
			"mac" | "macos" => String::from("mac"),
			s @ ("android32" | "android64") => String::from(s),
			s => fatal!("Unknown platform {s}"),
		})
		.unwrap_or_else(|| String::from("win"));
	let build_folder = if cross_compiling {
		format!("build-{platform}")
	} else {
		String::from("build")
	};

	let mut conf_args = Vec::new();
	if platform == "win" {
		conf_args.extend(["-A", "Win32"]);
	}

	let build_type = if platform == "win" {
		"RelWithDebInfo"
	} else {
		"Debug"
	};

	let status = Command::new("cmake")
		.args(["-B", &build_folder])
		.arg(format!("-DCMAKE_BUILD_TYPE={build_type}"))
		.args(conf_args)
		.spawn()
		.nice_unwrap("Failed to run cmake")
		.wait()
		.nice_unwrap("Failed to wait for cmake idk");
	if !status.success() {
		fail!("CMake returned code {}", status.code().unwrap_or(1));
		info!("Tip: deleting the build folder might help :-)");
		std::process::exit(1);
	}

	let status = Command::new("cmake")
		.args(["--build", &build_folder])
		.args(["--config", build_type])
		.spawn()
		.nice_unwrap("Failed to run cmake build")
		.wait()
		.nice_unwrap("Failed to wait for cmake build idk");
	if !status.success() {
		fatal!("CMake build returned code {}", status.code().unwrap_or(1));
	}

	info!("okay it probably built");
}
