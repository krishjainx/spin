use std::{
    collections::HashMap,
    env,
    path::Path,
    process::{self, Command},
};

use cargo_target_dep::build_target_dep;

const TIMER_TRIGGER_INTEGRATION_TEST: &str = "examples/spin-timer/app-example";

fn main() {
    std::env::remove_var("CARGO_ENCODED_RUSTFLAGS");

    if let Err(e) = vergen::EmitBuilder::builder()
        .build_date()
        .build_timestamp()
        .cargo_target_triple()
        .cargo_debug()
        .git_branch()
        .git_commit_date()
        .git_commit_timestamp()
        .git_sha(true)
        .emit()
    {
        eprintln!("Error extracting build information: {:?}", e);
        process::exit(1);
    }

    let build_spin_tests = env::var("BUILD_SPIN_EXAMPLES")
        .map(|v| v == "1")
        .unwrap_or(true);
    println!("cargo:rerun-if-env-changed=BUILD_SPIN_EXAMPLES");

    if !build_spin_tests {
        return;
    }

    println!("cargo:rerun-if-changed=build.rs");

    if !has_wasm32_wasi_target() {
        let current_toolchain = env::var("RUSTUP_TOOLCHAIN").unwrap_or_default();
        let current_toolchain = current_toolchain.split_once('-').map_or_else(|| "", |(toolchain, _)| toolchain);

        let default_toolchain = run(vec!["rustup", "default"], None, None);
        let default_toolchain = std::str::from_utf8(&default_toolchain.stdout).unwrap_or("");
        let default_toolchain = default_toolchain.split(['-', ' ']).next().unwrap_or("");

        let toolchain_override = if current_toolchain != default_toolchain {
            format!(" +{}", current_toolchain)
        } else {
            String::new()
        };

        eprintln!(
            "error: the `wasm32-wasi` target is not installed
            = help: consider downloading the target with `rustup{} target add wasm32-wasi`",
            toolchain_override
        );
        process::exit(1);
    }

    if let Err(e) = std::fs::create_dir_all("target/test-programs") {
        eprintln!("Failed to create directory: {:?}", e);
        process::exit(1);
    }

    build_wasm_test_program("core-wasi-test.wasm", "crates/core/tests/core-wasi-test");
    build_wasm_test_program("redis-rust.wasm", "crates/trigger-redis/tests/rust");

    build_wasm_test_program(
        "spin-http-benchmark.wasm",
        "crates/trigger-http/benches/spin-http-benchmark",
    );
    build_wasm_test_program(
        "wagi-benchmark.wasm",
        "crates/trigger-http/benches/wagi-benchmark",
    );
    build_wasm_test_program("timer_app_example.wasm", "examples/spin-timer/app-example");

    cargo_build(TIMER_TRIGGER_INTEGRATION_TEST);
}

fn build_wasm_test_program(name: &'static str, root: &'static str) {
    build_target_dep(root, Path::new("target/test-programs").join(name))
        .release()
        .target("wasm32-wasi")
        .build();
    println!("cargo:rerun-if-changed={root}/Cargo.toml");
    println!("cargo:rerun-if-changed={root}/Cargo.lock");
    println!("cargo:rerun-if-changed={root}/src");
}

fn has_wasm32_wasi_target() -> bool {
    let output = run(
        vec!["rustc", "--print=target-libdir", "--target=wasm32-wasi"],
        None,
        None,
    );
    match std::str::from_utf8(&output.stdout) {
        Ok(output) if !output.trim().is_empty() => std::path::Path::new(output.trim()).exists(),
        _ => false
    }
}

fn cargo_build(dir: &str) {
    run(
        vec![
            "cargo",
            "build",
            "--target",
            "wasm32-wasi",
            "--release",
            "--target-dir",
            "./target",
        ],
        Some(dir),
        None,
    );
    println!("cargo:rerun-if-changed={dir}/Cargo.toml");
    println!("cargo:rerun-if-changed={dir}/src");
}

fn run<S: Into<String> + AsRef<std::ffi::OsStr>>(
    args: Vec<S>,
    dir: Option<S>,
    env: Option<HashMap<S, S>>,
) -> process::Output {
    let mut cmd = Command::new(get_os_process());
    cmd.stdout(process::Stdio::piped());
    cmd.stderr(process::Stdio::piped());

    if let Some(dir) = dir.as_ref() {
        cmd.current_dir(dir);
    };

    if let Some(env) = env {
        for (k, v) in env {
            cmd.env(k, v);
        }
    };

    cmd.args(args);

    let output = cmd.output().expect("Failed to execute command");
    if !output.status.success() {
        let stderr = std::str::from_utf8(&output.stderr).unwrap_or("<unknown error>");
        let stdout = std::str::from_utf8(&output.stdout).unwrap_or("<unknown error>");
        panic!("Command failed with error: {}\nOutput: {}", stderr, stdout);
    }

    output
}

fn get_os_process() -> String {
    if cfg!(target_os = "windows") {
        "powershell.exe".into()
    } else {
        "bash".into()
    }
}

fn current_dir() -> String {
    env::current_dir()
        .map(|d| d.display().to_string())
        .unwrap_or_else(|_| "<CURRENT DIR>".into())
}
