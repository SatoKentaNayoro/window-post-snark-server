use anyhow::{Context};
use clap::{App, Arg};
use log::{error, info, warn};
use std::env;
use std::{process, process::exit};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use signal_hook::consts::TERM_SIGNALS;
use signal_hook::flag;
use tokio::sync::{mpsc, oneshot};
use window_post_snark_server::{server, server::WindowPostSnarkServer, utils, tasks};

fn main() {
    utils::set_commit_env();
    let cmds = App::new("window-post-snark-server")
        .author(utils::author())
        .version(utils::version())
        .subcommands(vec![run_cmd(), stop_cmd()]);
    let mut c = cmds.clone();
    let matches = cmds.get_matches();
    match matches.subcommand_name() {
        Some("run") => {
            env::set_var("RUST_BACKTRACE", "full");
            let run_matched = matches.subcommand_matches("run").unwrap();
            if run_matched.is_present("debug") {
                env::set_var("RUST_LOG", "debug");
            } else {
                env::set_var("RUST_LOG", "info");
            }

            fil_logger::init();
            let port = run_matched.value_of("port").unwrap().to_string();
            if run_matched.is_present("force") {
                run(port, true)
            } else {
                run(port, false)
            }
        }
        Some("stop") => {
            let stop_matched = matches.subcommand_matches("stop").unwrap();
            let pid = stop_matched.value_of("pid").unwrap().to_string();
            stop(pid);
        }
        _ => {
            c.print_help().unwrap();
            exit(1)
        }
    }
}

fn run_cmd() -> App<'static, 'static> {
    App::new("run").about("run window-post-snark-server").args(&[
        Arg::from_usage("-d, --debug 'print debug log'").required(false),
        Arg::from_usage("-f, --force 'force run process without num limit'").required(false),
        Arg::from_usage("-p, --port=[PORT] 'specify server port'")
            .default_value("50051")
            .required(false),
    ])
}

fn stop_cmd() -> App<'static, 'static> {
    App::new("stop").about("stop window-post-snark-server").arg(
        Arg::from_usage("-p, --pid=[PID] 'specify server pid'")
            .default_value("")
            .required(false),
    )
}


fn run(port: String, is_force: bool) {
    assert_eq!(can_run(is_force), true);
    let rt = tokio::runtime::Runtime::new().with_context(|| "failed to build new runtime").unwrap();
    // listening server exit signal
    let (server_exit_tx, server_exit_rx) = oneshot::channel::<String>();
    // listening task runner exit signal
    let (task_exit_tx, task_exit_rx) = oneshot::channel::<String>();

    let (run_task_tx, run_task_rx) = mpsc::unbounded_channel::<String>();

    let sv = WindowPostSnarkServer::default(run_task_tx);
    let sv_i = sv.server_info.clone();

    let sv_handle = rt.spawn(server::run_server(server_exit_rx, sv, port));

    let task_handle = rt.spawn(tasks::run_task(task_exit_rx, run_task_rx, sv_i));

    // listen exit signal
    rt.block_on(listen_exit_signal());

    // stop task
    match task_exit_tx.send("exit".to_string()) {
        Ok(_) => {}
        Err(e) => {
            error!("{}",e);
            return;
        }
    };

    // wait task stop
    rt.block_on(
        async {
            match task_handle.await {
                Ok(_) => {}
                Err(e) => {
                    error!("{}",e)
                }
            }
        }
    );

    // send sig to stop server
    match server_exit_tx.send("exit".to_string()) {
        Ok(_) => {}
        Err(e) => {
            error!("{}",e);
            return;
        }
    };

    // wait server exit
    rt.block_on(
        async {
            match sv_handle.await {
                Ok(_) => {}
                Err(e) => {
                    error!("{}",e)
                }
            }
        }
    );

    // del file lock
    utils::del_file_lock();
    rt.shutdown_background();
    info!("server main process exited")
}

fn can_run(is_force: bool) -> bool {
    if !is_force {
        if utils::is_file_lock_exist() {
            warn!("file lock existed,will check process is_running by pid");
            if let Some(p) = utils::check_process_is_running_by_pid() {
                error!("process double run, old process still running, pid: {}", p);
                false
            } else {
                warn!("old process is not running, let's go on");
                true
            }
        } else {
            let pid = &process::id().to_string().as_bytes().to_vec();
            match utils::write_pid_into_file_lock(pid) {
                Ok(_) => {
                    info!("write pid into lock file success");
                    true
                }
                Err(e) => {
                    error!("write pid into lock file failed with error:{}", e);
                    false
                }
            }
        }
    } else {
        true
    }
}

fn stop(p: String) {
    println!("{}", p)
}

async fn listen_exit_signal() {
    let term = Arc::new(AtomicBool::new(false));
    for sig in TERM_SIGNALS {
        match flag::register(*sig, Arc::clone(&term)) {
            Ok(_) => {}
            Err(e) => {
                error!("failed to register TERM_SIGNALS with error:{}",e);
                return;
            }
        };
    }
    while !term.load(Ordering::Relaxed) {
        tokio::time::sleep(Duration::new(1, 0)).await;
    }
}