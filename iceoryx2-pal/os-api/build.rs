// Copyright (c) 2025 Contributors to the Eclipse Foundation
//
// See the NOTICE file(s) distributed with this work for additional
// information regarding copyright ownership.
//
// This program and the accompanying materials are made available under the
// terms of the Apache Software License 2.0 which is available at
// https://www.apache.org/licenses/LICENSE-2.0, or the MIT license
// which is available at https://opensource.org/licenses/MIT.
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

#[cfg(feature = "libc_platform")]
fn main() {}

const MANUAL_EXTRACT_BINDGEN: [&str; 1] = ["ebclfsa"];

#[cfg(not(feature = "libc_platform"))]
fn main() {
    // when cross compiling, 'target_os' is set to the environment the build script
    // is executed; to get the actual target OS, use the cargo 'CARGO_CFG_TARGET_OS' env variable
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap();
    println!("Building for target: {}", target_os);

    // the check for 'linux' in the next line refers to native compilation
    // and prevents to pull in bindgen
    #[cfg(target_os = "linux")]
    // the check for 'linux' in the next line refers to cross compilation
    if target_os == "linux" {
        extern crate bindgen;
        extern crate cc;

        use bindgen::*;
        use std::env;
        use std::path::PathBuf;

        println!("cargo:rerun-if-changed=src/c/linux.h");

        let mut builder = get_sysroot_and_isystem_paths(bindgen::Builder::default())
            .header("src/c/linux.h")
            .parse_callbacks(Box::new(CargoCallbacks::new()))
            .use_core();

        if std::env::var("DOCS_RS").is_ok() {
            builder = builder.clang_arg("-D IOX2_DOCS_RS_SUPPORT");
        }

        let bindings = builder.generate().expect("Unable to generate bindings");

        // needed for bazel but can be empty for cargo builds
        println!("cargo:rustc-env=BAZEL_BINDGEN_PATH_CORRECTION=");

        let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
        bindings
            .write_to_file(out_path.join("os_api_generated.rs"))
            .expect("Couldn't write bindings!");
    }
}


fn get_sysroot_and_isystem_paths(builder: bindgen::Builder) -> bindgen::Builder{
    let cc = std::env::var("CC") .unwrap_or_else(|_| "cc".to_string());
    let shall_extract = MANUAL_EXTRACT_BINDGEN.iter().any(|&entry| cc.contains(entry));
    if !shall_extract {
        return builder;
    }

    let cflags = std::env::var("CFLAGS").unwrap_or_else(|_| "cc".to_string());

    let tokens: Vec<&str> = cflags.split_whitespace().collect();
    let mut iter = tokens.iter().peekable();
    let mut sysroot = None;
    let mut isystems = Vec::new();

    while let Some(&&tok) = iter.peek() {
        if tok == "-isystem" {
            iter.next(); // consume "-isystem"
            if let Some(path) = iter.next() {
                isystems.push(path.to_string());
            }
        } else if tok.starts_with("-isystem") {
            // handle concatenated form: -isystem/path
            let path = tok.trim_start_matches("-isystem");
            isystems.push(path.to_string());
            iter.next();
        } else if tok.starts_with("--sysroot=") {
            let path = tok.trim_start_matches("--sysroot=");
            sysroot = Some(path.to_string());
            iter.next();
        } else {
            iter.next();
        }
    }

    let target: &'static str = "execroot/_main";
    let mut repl = None;
    if let Some(pos) = cc.find(target) {
        repl = Some(&cc[..pos + target.len()]);
    }

    let mut ld = std::env::var("LD_LIBRARY_PATH") .unwrap_or_else(|_| "cc".to_string());

    ld = ld.replace("/proc/self/cwd", repl.expect("Failed to find execroot in CC path - shall this toolchain have this patching at all?!"));
    std::env::set_var("LD_LIBRARY_PATH", ld);

    ld = std::env::var("LD_LIBRARY_PATH") .unwrap_or_else(|_| "cc".to_string());
    println!("LD_LIBRARY_PATH after correction: {}", ld);

    for path in &isystems {
        println!("Extended isystem for clang = manual patch: {}", path);
    }

    if let Some(root) = &sysroot {
        println!("Extended sysroot for clang = manual patch: {}", root);
    }

    builder.clang_arg("-v").clang_args(isystems.iter().map(|path| format!("-I{}", path))).clang_args(sysroot.iter().map(|root| format!("--sysroot={}", root)))
}
