use std::{path::Path, process::Command};

use crate::{fail, fatal, info, mod_file::PlatformName, warn, NiceUnwrap};

pub fn build_project(
    platform: Option<PlatformName>,
    configure_only: bool,
    build_only: bool,
    ndk_path: Option<String>,
    config_type: Option<String>,
    extra_conf_args: Vec<String>,
) {
    if !Path::new("CMakeLists.txt").exists() {
        fatal!("Could not find CMakeLists.txt. Please run this within a Geode project!");
    }

    let platform = platform.unwrap_or_else(|| {
        PlatformName::current().nice_unwrap("Unknown platform, please specify one with --platform")
    });

    // Make architechture exact
    let platform = match platform {
        PlatformName::Android => {
            warn!("Assuming 64-bit Android, use \"-p android32\" to build for 32-bit Android");
            PlatformName::Android64
        }
        // If Mac cross-building ever becomes possible, make sure to upgrade Mac
        // to MacArm or MacIntel here (or hard error if there's no reasonable
        // default)
        p => p,
    };

    let cross_compiling = if cfg!(target_os = "windows") {
        platform != PlatformName::Windows
    } else if cfg!(target_os = "linux") {
        true
    } else if cfg!(target_os = "macos") {
        platform != PlatformName::MacOS
    } else {
        true
    };

    let build_folder = if cross_compiling {
        format!("build-{platform}")
    } else {
        String::from("build")
    };

    let mut conf_args: Vec<String> = Vec::new();
    match platform {
        PlatformName::Windows => {
            if cross_compiling {
                let root = crate::config::Config::cross_tools_path();
                let splat_path = root.join("splat");
                let toolchain_path = root.join("clang-msvc-sdk");

                if !extra_conf_args
                    .iter()
                    .any(|arg| arg.contains("-DCMAKE_TOOLCHAIN_FILE"))
                {
                    conf_args.push(format!(
                        "-DCMAKE_TOOLCHAIN_FILE={}",
                        toolchain_path.join("clang-msvc.cmake").to_string_lossy()
                    ));
                }

                if !extra_conf_args
                    .iter()
                    .any(|arg| arg.contains("-DSPLAT_DIR"))
                {
                    conf_args.push(format!("-DSPLAT_DIR={}", splat_path.to_string_lossy()));
                }

                conf_args.push("-DHOST_ARCH=x64".to_owned());
            } else {
                conf_args.extend(["-A", "x64"].map(String::from));
            }
        }
        PlatformName::MacOS | PlatformName::MacArm | PlatformName::MacIntel => {
            if cross_compiling {
                fatal!("Sorry! but we do not know of any way to cross-compile to MacOS.");
            }
            conf_args.push("-DCMAKE_OSX_DEPLOYMENT_TARGET=10.15".into())
        }
        PlatformName::Android32 | PlatformName::Android64 | PlatformName::Android => {
            if !build_only {
                let ndk_path = ndk_path.unwrap_or_else(||
					std::env::var("ANDROID_NDK_ROOT").nice_unwrap(
                        "Failed to get NDK path, either pass it through --ndk or set the ANDROID_NDK_ROOT enviroment variable"
                    )
                );
                let toolchain_path =
                    Path::new(ndk_path.as_str()).join("build/cmake/android.toolchain.cmake");
                if !toolchain_path.exists() {
                    fatal!("Invalid NDK path {ndk_path:?}, could not find toolchain");
                }
                conf_args.push(format!(
                    "-DCMAKE_TOOLCHAIN_FILE={}",
                    toolchain_path.to_string_lossy()
                ));
                if platform == PlatformName::Android32 {
                    conf_args.push("-DANDROID_ABI=armeabi-v7a".into());
                } else {
                    conf_args.push("-DANDROID_ABI=arm64-v8a".into());
                }
                // TODO: let the user change this? idk
                conf_args.push("-DANDROID_PLATFORM=23".into());
                if cfg!(target_os = "windows") && !extra_conf_args.contains(&"-G".to_owned()) {
                    conf_args.extend(["-G", "Ninja"].map(String::from));
                }
                conf_args.push("-DCMAKE_EXPORT_COMPILE_COMMANDS=1".into());
                // TODO: cli cant install to a mobile device, yet
                conf_args.push("-DGEODE_DONT_INSTALL_MODS=1".into());
            }
        }
    }

    let build_type = config_type.unwrap_or_else(|| {
        if platform == PlatformName::Windows {
            "RelWithDebInfo".into()
        } else {
            "Debug".into()
        }
    });

    if !build_only {
        // Configure project
        let status = Command::new("cmake")
            .args(["-B", &build_folder])
            .arg(format!("-DCMAKE_BUILD_TYPE={build_type}"))
            .args(conf_args)
            .args(extra_conf_args)
            .spawn()
            .nice_unwrap("Failed to run cmake")
            .wait()
            .nice_unwrap("Failed to wait for cmake idk");
        if !status.success() {
            fail!("CMake returned code {}", status.code().unwrap_or(1));
            info!("Tip: deleting the build folder might help :-)");
            std::process::exit(1);
        }
    }

    if !configure_only {
        // Build project
        let status = Command::new("cmake")
            .args(["--build", &build_folder])
            .args(["--config", build_type.as_str()])
            .spawn()
            .nice_unwrap("Failed to run cmake build")
            .wait()
            .nice_unwrap("Failed to wait for cmake build idk");
        if !status.success() {
            fatal!("CMake build returned code {}", status.code().unwrap_or(1));
        }
    }
}
