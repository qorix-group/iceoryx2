// Copyright (c) 2023 Contributors to the Eclipse Foundation
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
    extern crate bindgen;
    extern crate cc;

    use bindgen::*;
    use std::env;
    use std::path::PathBuf;

    // #[cfg(any(...))] does not work when cross-compiling
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap();
    if target_os == "linux" || target_os == "freebsd" {
        println!("cargo:rustc-link-lib=pthread");
    }

    println!("cargo:rerun-if-changed=src/c/posix.h");

    let mut builder = bindgen::Builder::default();
    builder = get_sysroot_and_isystem_paths(builder);

    builder = builder
        .header("src/c/posix.h")
        .blocklist_type("max_align_t")
        .parse_callbacks(Box::new(CargoCallbacks::new()))
        .use_core();

    if std::env::var("DOCS_RS").is_ok() {
        builder = builder.clang_arg("-D IOX2_DOCS_RS_SUPPORT");
    }

    if target_os == "nto" {
        let target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap();
        let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap();

        // Common compiler defines for QNX
        let mut compiler_args = vec![
            "-D__QNXNTO__",
            "-D__NO_INLINE__",
            "-D__DEPRECATED",
            "-D__unix__",
            "-D__unix",
            "-D__ELF__",
            "-D__LITTLEENDIAN__",
        ];

        // Version-specific compiler defines for QNX
        match target_env.as_str() {
            "nto71" => {
                compiler_args.push("-D__QNX__");
                compiler_args.push("-D__GNUC__=8");
                compiler_args.push("-D__GNUC_MINOR__=3");
                compiler_args.push("-D__GNUC_PATCHLEVEL__=0");
            }
            "nto80" => {
                compiler_args.push("-D__QNX__=800");
                compiler_args.push("-D__GNUC__=12");
                compiler_args.push("-D__GNUC_MINOR__=2");
                compiler_args.push("-D__GNUC_PATCHLEVEL__=0");
            }
            _ => {
                panic!(
                    "Unsupported QNX target environment: {target_env}. Only nto71 and nto80 are supported.",
                );
            }
        }

        // Architecture-specific compiler defines for QNX
        if target_arch == "x86_64" {
            compiler_args.push("-D__X86_64__");
        }

        for arg in &compiler_args {
            builder = builder.clang_arg(*arg);
        }

        if let Ok(sysroot) = env::var("QNX_TARGET") {
            builder = builder.clang_arg(format!("--sysroot={sysroot}"));
            builder = builder.clang_arg(format!("-I{sysroot}/usr/include"));
            builder = builder.clang_arg(format!("-I{sysroot}/usr/include/c++/v1"));
        } else {
            panic!("QNX_TARGET environment variable not set for QNX build")
        }
    }

    let bindings = builder.generate().expect("Unable to generate bindings");

    // needed for bazel but can be empty for cargo builds
    println!("cargo:rustc-env=BAZEL_BINDGEN_PATH_CORRECTION=");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("posix_generated.rs"))
        .expect("Couldn't write bindings!");

    println!("cargo:rerun-if-changed=src/c/socket_macros.c");
    cc::Build::new()
        .file("src/c/socket_macros.c")
        .compile("libsocket_macros.a");
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
