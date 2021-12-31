use clap::crate_version;
use std::env;
use std::process;

pub fn set_commit_env() {
    if let Ok(x) = process::Command::new("git")
        .args(&["rev-parse", "--short", "HEAD"])
        .output()
    {
        if x.status.success() {
            if let Ok(commit_id) = String::from_utf8(x.stdout) {
                env::set_var("PROJECT_VERSION", commit_id);
            }
        }
    }
}

pub fn author() -> &'static str {
    "IronC,https://github.com/hxuchen"
}

pub fn version() -> &'static str {
    let mut commit_id;
    if let Ok(c) = env::var("PROJECT_VERSION") {
        commit_id = c
    } else {
        commit_id = "".to_string()
    }

    Box::leak(format!("{}+git.{}", crate_version!(), commit_id).into_boxed_str())
}
