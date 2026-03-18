use std::{env, process::Command};
fn main() {
    let sha = env::var("GIT_SHA").ok().unwrap_or_else(|| {
        Command::new("git")
            .args(["rev-parse", "--short=12", "HEAD"])
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|| "unknown".into())
    });
    let build_time = env::var("BUILD_TIME").unwrap_or_else(|_| chrono::Utc::now().to_rfc3339());
    println!("cargo:rustc-env=GIT_SHA={sha}");
    println!("cargo:rustc-env=BUILD_TIME={build_time}");
}
