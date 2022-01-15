use crate::server::{
    WindowPostSnarkServer, SERVER_EXIT_TIME_OUT_AFTER_TASK_DONE_DEFAULT,
    SERVER_LOCK_TIME_OUT_DEFAULT, SERVER_TASK_GET_BACK_TIME_OUT_DEFAULT,
};
use crate::{server, tasks, utils};
use anyhow::Context;
use log::{debug, error, info};
use signal_hook::consts::TERM_SIGNALS;
use signal_hook::flag;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};

pub fn run(
    port: String,
    server_lock_time_out: Duration,
    server_task_get_back_time_out: Duration,
    server_exit_time_out_after_task_done: Duration,
) {
    let rt = tokio::runtime::Runtime::new()
        .with_context(|| "failed to build new runtime")
        .unwrap();
    // listening server exit signal
    let (server_exit_tx, server_exit_rx) = oneshot::channel::<String>();
    // listening task runner exit signal
    let (task_exit_tx, task_exit_rx) = oneshot::channel::<String>();

    let (run_task_tx, run_task_rx) = mpsc::unbounded_channel::<String>();

    let sv = WindowPostSnarkServer::new(run_task_tx);

    if server_lock_time_out != SERVER_LOCK_TIME_OUT_DEFAULT
        && server_task_get_back_time_out != SERVER_TASK_GET_BACK_TIME_OUT_DEFAULT
        && server_exit_time_out_after_task_done != SERVER_EXIT_TIME_OUT_AFTER_TASK_DONE_DEFAULT
    {
        sv.set_time_out(
            server_lock_time_out,
            server_task_get_back_time_out,
            server_exit_time_out_after_task_done,
        )
        .unwrap();
    } else {
        if server_lock_time_out != SERVER_LOCK_TIME_OUT_DEFAULT {
            sv.set_server_lock_time_out(server_lock_time_out).unwrap();
        }
        if server_task_get_back_time_out != SERVER_TASK_GET_BACK_TIME_OUT_DEFAULT {
            sv.set_server_task_get_back_time_out(server_task_get_back_time_out)
                .unwrap();
        }
        if server_exit_time_out_after_task_done != SERVER_EXIT_TIME_OUT_AFTER_TASK_DONE_DEFAULT {
            sv.set_server_exit_time_out_after_task_done(server_exit_time_out_after_task_done)
                .unwrap();
        }
    };

    debug!("server_info:{:?}", sv.server_info);

    let sv_i = sv.server_info.clone();

    let sv_handle = rt.spawn(server::run_server(server_exit_rx, sv, port));

    let task_handle = rt.spawn(tasks::run_task(task_exit_rx, run_task_rx, sv_i));

    // listen exit signal
    rt.block_on(listen_exit_signal());

    // stop task
    match task_exit_tx.send("exit".to_string()) {
        Ok(_) => {}
        Err(e) => {
            error!("{}", e);
            return;
        }
    };

    // wait task stop
    rt.block_on(async {
        match task_handle.await {
            Ok(_) => {}
            Err(e) => {
                error!("{}", e)
            }
        }
    });

    // send sig to stop server
    match server_exit_tx.send("exit".to_string()) {
        Ok(_) => {}
        Err(e) => {
            error!("{}", e);
            return;
        }
    };

    // wait server exit
    rt.block_on(async {
        match sv_handle.await {
            Ok(_) => {}
            Err(e) => {
                error!("{}", e)
            }
        }
    });

    // del file lock
    utils::del_file_lock();
    rt.shutdown_background();
    info!("server main process exited")
}

async fn listen_exit_signal() {
    let term = Arc::new(AtomicBool::new(false));
    for sig in TERM_SIGNALS {
        match flag::register(*sig, Arc::clone(&term)) {
            Ok(_) => {}
            Err(e) => {
                error!("failed to register TERM_SIGNALS with error:{}", e);
                return;
            }
        };
    }
    while !term.load(Ordering::Relaxed) {
        tokio::time::sleep(Duration::new(1, 0)).await;
    }
}
