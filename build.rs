use sha2::{Digest, Sha256};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn collect(directory: &Path, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(directory).expect("read source directory") {
        let path = entry.expect("source entry").path();
        if path.is_dir() {
            collect(&path, files);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn main() {
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest directory"));
    let mut files = vec![root.join("Cargo.toml"), root.join("build.rs")];
    let lock = root.join("Cargo.lock");
    if lock.is_file() {
        files.push(lock.clone());
    }
    collect(&root.join("src"), &mut files);
    files.sort_by_key(|path| {
        path.strip_prefix(&root)
            .unwrap()
            .to_str()
            .expect("UTF-8 source path")
            .replace('\\', "/")
    });
    let mut source = Sha256::new();
    source.update(b"nmr.source-files.v1\0");
    for file in files {
        let relative = file
            .strip_prefix(&root)
            .unwrap()
            .to_str()
            .unwrap()
            .replace('\\', "/");
        println!("cargo:rerun-if-changed={relative}");
        let bytes = fs::read(&file).expect("read compilation input");
        source.update((relative.len() as u64).to_le_bytes());
        source.update(relative.as_bytes());
        source.update((bytes.len() as u64).to_le_bytes());
        source.update(bytes);
    }
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=Cargo.lock");
    let source_hash = hex(&source.finalize());
    let lock_hash = fs::read(lock).ok().map(|bytes| hex(&Sha256::digest(bytes)));
    let rustc = Command::new(env::var_os("RUSTC").expect("Rust compiler"))
        .arg("-vV")
        .output()
        .expect("query compiler identity");
    assert!(rustc.status.success(), "query compiler identity");
    let rustc = String::from_utf8(rustc.stdout)
        .expect("compiler identity UTF-8")
        .replace(['\r', '\n'], " ")
        .trim()
        .to_owned();
    let mut build = Sha256::new();
    build.update(b"nmr.build-inputs.v1\0");
    build.update(source_hash.as_bytes());
    build.update(rustc.as_bytes());
    for name in [
        "TARGET",
        "PROFILE",
        "OPT_LEVEL",
        "DEBUG",
        "CARGO_CFG_TARGET_FEATURE",
        "CARGO_ENCODED_RUSTFLAGS",
    ] {
        println!("cargo:rerun-if-env-changed={name}");
        let value = env::var(name).unwrap_or_default();
        build.update((name.len() as u64).to_le_bytes());
        build.update(name.as_bytes());
        build.update((value.len() as u64).to_le_bytes());
        build.update(value.as_bytes());
        if name != "CARGO_ENCODED_RUSTFLAGS" {
            println!("cargo:rustc-env=NMR_BUILD_{name}={value}");
        } else {
            println!(
                "cargo:rustc-env=NMR_RUSTFLAGS_UTF8_HEX={}",
                hex(value.as_bytes())
            );
        }
    }
    println!("cargo:rustc-env=NMR_SOURCE_SHA256={source_hash}");
    println!(
        "cargo:rustc-env=NMR_BUILD_SHA256={}",
        hex(&build.finalize())
    );
    println!(
        "cargo:rustc-env=NMR_PACKAGED_LOCK_SHA256={}",
        lock_hash.as_deref().unwrap_or("unknown")
    );
    println!("cargo:rustc-env=NMR_RUSTC_IDENTITY={rustc}");
}
