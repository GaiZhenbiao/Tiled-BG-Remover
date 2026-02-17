use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn patch_psd_cpp_layer_coordinates(source_dir: &Path) {
    let layer_header = source_dir.join("headers/psd/document/layer.h");
    let Ok(contents) = fs::read_to_string(&layer_header) else {
        return;
    };

    let patched = contents
        .replace("      xoffset_ + image_.RowCount(),", "      yoffset_ + image_.RowCount(),")
        .replace(
            "      yoffset_ + image_.ColumnCount()",
            "      xoffset_ + image_.ColumnCount()",
        );

    if patched != contents {
        let _ = fs::write(layer_header, patched);
    }
}

fn main() {
    let output_dir = env::var("OUT_DIR").unwrap();
    let source_dir = Path::new(&output_dir).join("psd-cpp");
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();

    if !source_dir.exists() {
        let status = Command::new("git")
            .args(&[
                "clone",
                "https://github.com/weqeqq/psd-cpp",
                source_dir.to_str().unwrap(),
            ])
            .status()
            .expect("Failed to run git clone");
        assert!(status.success(), "git clone failed");
    }
    patch_psd_cpp_layer_coordinates(&source_dir);
    if target_os == "windows" {
        // Use the Visual Studio generator directly on Windows so CMake resolves MSVC
        // toolchain itself (without requiring cl.exe in PATH), and avoid mixed-toolchain
        // object issues that can cause LNK1143 in CI.
        let build_dir = source_dir.join("build").join("release-msvc");
        let mut configure = Command::new("cmake");
        configure
            .arg("-S")
            .arg(&source_dir)
            .arg("-B")
            .arg(&build_dir)
            .arg("-G")
            .arg("Visual Studio 17 2022")
            .arg("-A")
            .arg("x64")
            .arg(format!("-DCMAKE_INSTALL_PREFIX={output_dir}"))
            .arg("-DCMAKE_INTERPROCEDURAL_OPTIMIZATION=OFF");
        assert!(
            configure.status().unwrap().success(),
            "cmake configure failed for psd-cpp"
        );
        assert!(
            Command::new("cmake")
                .arg("--build")
                .arg(&build_dir)
                .arg("--config")
                .arg("Release")
                .arg("--target")
                .arg("install")
                .status()
                .unwrap()
                .success(),
            "cmake build/install failed for psd-cpp"
        );
    } else {
        env::set_current_dir(source_dir).unwrap();
        let mut configure = Command::new("cmake");
        configure.args(&[
            "--preset",
            "release",
            format!("-DCMAKE_INSTALL_PREFIX={}", output_dir).as_str(),
        ]);
        if target_os == "macos" {
            // Xcode 16 marks std::filesystem APIs unavailable below 10.15, and arm64 apps
            // are expected to target macOS 11+.
            let deployment_target = match env::var("MACOSX_DEPLOYMENT_TARGET") {
                Ok(value) => {
                    let mut parts = value.split('.');
                    let major = parts
                        .next()
                        .and_then(|p| p.parse::<u32>().ok())
                        .unwrap_or(11);
                    if major < 11 {
                        "11.0".to_string()
                    } else {
                        value
                    }
                }
                Err(_) => "11.0".to_string(),
            };
            configure.env("MACOSX_DEPLOYMENT_TARGET", &deployment_target);
            configure.arg(format!(
                "-DCMAKE_OSX_DEPLOYMENT_TARGET={deployment_target}"
            ));
        }
        assert!(
            configure.status().unwrap().success(),
            "cmake configure failed for psd-cpp"
        );
        assert!(
            Command::new("cmake")
                .args(&["--build", "--preset", "release", "--target", "install"])
                .status()
                .unwrap()
                .success(),
            "cmake build/install failed for psd-cpp"
        );
    }

    println!(
        "cargo:rustc-link-search=native={}",
        PathBuf::from(output_dir).join("lib").display()
    );
    println!("cargo:rustc-link-lib=static=psd");
    println!("cargo:rustc-link-lib=static=file");
    if target_os == "windows" {
        // With MSVC generator libdeflate is installed as libdeflate.lib.
        println!("cargo:rustc-link-lib=static=libdeflate");
    } else {
        println!("cargo:rustc-link-lib=static=deflate");
    }
    if target_os == "macos" {
        println!("cargo:rustc-link-lib=dylib=c++");
    } else if target_os != "windows" {
        println!("cargo:rustc-link-lib=dylib=stdc++");
    }
    println!("cargo:rerun-if-changed=build.rs");
}
