fn main() {
    // Expose the Cargo target triple to the supervisor binary so it can create
    // correctly-named sidecar stub files in new worktrees at runtime.
    if let Ok(target) = std::env::var("TARGET") {
        println!("cargo:rustc-env=CARGO_BUILD_TARGET={target}");
    }
    tauri_build::build()
}
