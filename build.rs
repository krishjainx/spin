use std::{
    collections::HashMap,
    env,
    path::Path,
    process::{self, Command},
};

const TIMER_TRIGGER_INTEGRATION_TEST: &str = "examples/spin-timer/app-example";

fn main() {
    // Don't inherit flags from our own invocation of cargo into sub-invocations
    // since the flags are intended for the host and we're compiling for wasm.
    std::env::remove_var("CARGO_ENCODED_RUSTFLAGS");

    // Extract environment information to be passed to plugins.
    // Git information will be set to defaults if Spin is not
    // built within a Git worktree.
    vergen::EmitBuilder::builder()
        .build_date()
        .build_timestamp()
        .cargo_target_triple()
        .cargo_debug()
        .git_branch()
        .git_commit_date()
        .git_commit_timestamp()
        .git_sha(true)
        .emit()
        .expect("failed to extract build information");

    let build_spin_tests = env::var("BUILD_SPIN_EXAMPLES")
        .map(|v| v == "1")
        .unwrap_or(false);
    println!("cargo:rerun-if-env-changed=BUILD_SPIN_EXAMPLES");

    if !build_spin_tests {
        return;
    }

    println!("cargo:rerun-if-changed=build.rs");

    if !has_wasm32_wasi_target() {
        println!("error: the `wasm32-wasi` target is not installed");
        process::exit(1);
    }

    std::fs::create_dir_all("target/test-programs").unwrap();

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
    let output = Command::new("rustc")
        .args(["--print=target-libdir", "--target=wasm32-wasi"])
        .output();

    match output {
        Ok(output) if output.status.success() => {
            let path_str = String::from_utf8_lossy(&output.stdout);
            !path_str.trim().is_empty() && Path::new(path_str.trim()).exists()
        },
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
            // Ensure that even if `CARGO_TARGET_DIR` is set
            // that we're still building into the right dir.
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

    if let Some(ref d) = dir {
        cmd.current_dir(d.as_ref());
    }

    if let Some(ref e) = env {
        for (key, value) in e {
            cmd.env(key.as_ref(), value.as_ref());
        }
    }

    cmd.args(args.iter().map(AsRef::as_ref));

    let output = cmd.output();

    match output {
        Ok(output) if output.status.success() => output,
        Ok(output) => panic!(
            "Command execution failed with output: {} and error: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ),
        Err(e) => panic!("Failed to execute command: {:?}", e),
    }
}

fn get_os_process() -> String {
    if cfg!(target_os = "windows") {
        String::from("powershell.exe")
    } else {
        String::from("bash")
    }
}