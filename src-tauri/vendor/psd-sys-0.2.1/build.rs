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
    env::set_current_dir(source_dir).unwrap();
    assert!(
        Command::new("cmake")
            .args(&[
                "--preset",
                "release",
                format!("-DCMAKE_INSTALL_PREFIX={}", output_dir).as_str()
            ])
            .status()
            .unwrap()
            .success(),
        "huy"
    );
    assert!(
        Command::new("cmake")
            .args(&["--build", "--preset", "release", "--target", "install"])
            .status()
            .unwrap()
            .success(),
        "huy"
    );

    println!(
        "cargo:rustc-link-search=native={}",
        PathBuf::from(output_dir).join("lib").display()
    );
    println!("cargo:rustc-link-lib=static=psd");
    println!("cargo:rustc-link-lib=static=file");
    println!("cargo:rustc-link-lib=static=deflate");
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "macos" {
        println!("cargo:rustc-link-lib=dylib=c++");
    } else if target_os != "windows" {
        println!("cargo:rustc-link-lib=dylib=stdc++");
    }
    println!("cargo:rerun-if-changed=build.rs");
}
