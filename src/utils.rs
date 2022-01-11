use clap::crate_version;
use log::{error, info};
use std::env;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::{
    fs::{remove_file, write},
    process,
};

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
    if let Ok(c) = env::var("PROJECT_VERSION") {
        Box::leak(format!("{}+git.{}", crate_version!(), c).into_boxed_str())
    } else {
        Box::leak(format!("{}+git.{}", crate_version!(), "".to_string()).into_boxed_str())
    }
}

pub fn lock_file_path() -> PathBuf {
    dirs::home_dir().unwrap().join(".fil_wdpost_server.lock")
}

pub fn is_file_lock_exist() -> bool {
    let lock_path = lock_file_path();
    info!("lock_path: {:?}", lock_path.clone().as_path());
    Path::new(lock_path.as_path()).exists()
}

pub fn write_pid_into_file_lock(pid: &Vec<u8>) -> Result<(), anyhow::Error> {
    let lock_path = lock_file_path();
    let result = write(lock_path, pid);
    match result {
        Ok(_) => Ok(()),
        Err(e) => Err(anyhow::Error::msg(e.to_string())),
    }
}

pub fn check_process_is_running_by_pid() -> Option<u32> {
    let lock_path = lock_file_path().to_str().unwrap().to_string();
    let pid = read_pid(lock_path);
    if pid == 0 {
        None
    } else {
        let pid_str = pid.to_string();
        let args = vec!["-p", &pid_str, "-o", "pid="];
        let ps_cmd_out = process::Command::new("ps")
            .args(args)
            .output()
            .expect("failed to execute ps -p");
        if ps_cmd_out.status.success() {
            if String::from_utf8(ps_cmd_out.stdout)
                .unwrap()
                .contains(&pid_str.to_string())
            {
                Some(pid)
            } else {
                None
            }
        } else {
            None
        }
    }
}

pub fn del_file_lock() {
    let lock_path = lock_file_path();
    match remove_file(lock_path) {
        Ok(_) => {}
        Err(e) => {
            error!("{}", e);
        }
    };
}

pub fn read_pid(path: String) -> u32 {
    match File::open(path) {
        Ok(data) => {
            let mut buf_reader = BufReader::new(data);
            let mut contents = String::new();
            buf_reader
                .read_to_string(&mut contents)
                .expect("read pid failed");
            contents
                .parse::<u32>()
                .expect("parse pid error from pid file")
        }
        Err(_) => 0,
    }
}
