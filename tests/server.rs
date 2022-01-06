use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use anyhow::{Result};
use log::error;
use signal_hook::consts::TERM_SIGNALS;
use signal_hook::flag;
use tokio::runtime::Runtime;
use tokio::sync::{mpsc, oneshot};
use tonic::Request;
use uuid::Uuid;
use window_post_snark_server::server;
use window_post_snark_server::server::WindowPostSnarkServer;
use window_post_snark_server::client;
use window_post_snark_server::snark_proof_grpc::{GetWorkerStatusRequest, UnlockServerRequest};

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
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

fn run_s() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (run_task_tx, _) = mpsc::unbounded_channel::<String>();
    let (server_exit_tx, server_exit_rx) = oneshot::channel::<String>();
    let sv = WindowPostSnarkServer::default(run_task_tx);
    let handle = rt.spawn(server::run_server(server_exit_rx, sv, "50051".to_string()));

    rt.block_on(listen_exit_signal());
    server_exit_tx.send("exit".to_string()).unwrap();
    rt.block_on(async { handle.await.unwrap() });
    rt.shutdown_background();
}


#[test]
fn test_run_server() -> Result<()> {
    fil_logger::init();
    run_s();
    Ok(())
}


#[test]
fn test_lock_server_if_free() -> Result<()> {
    let rt = Runtime::new().unwrap();
    let mut c = rt.block_on(client::new_client("http://127.0.0.1:50051", Duration::from_secs(10))).unwrap();
    let mut times = 1;
    loop {
        if times >= 20 {
            break;
        }
        let task_id = Uuid::new_v4().to_string();
        let req = Request::new(GetWorkerStatusRequest { task_id });
        rt.block_on(async {
            match c.lock_server_if_free(req).await {
                Ok(res) => {
                    println!("{}", res.into_inner().msg)
                }
                Err(s) => {
                    println!("{}", s.message())
                }
            }
        });
        times += 1;
        // rt.block_on(async {
        //     tokio::time::sleep(Duration::from_secs(times)).await
        // })
    }
    rt.block_on(async { tokio::time::sleep(Duration::from_secs(10)).await });
    let task_id = Uuid::new_v4().to_string();
    let req = Request::new(GetWorkerStatusRequest { task_id });
    rt.block_on(async {
        match c.lock_server_if_free(req).await {
            Ok(res) => {
                println!("{}", res.into_inner().msg)
            }
            Err(s) => {
                println!("{}", s.message())
            }
        }
    });
    Ok(())
}

#[test]
fn test_unlock_server() -> Result<()> {
    let rt = Runtime::new().unwrap();
    let mut c = rt.block_on(client::new_client("http://127.0.0.1:50051", Duration::from_secs(10))).unwrap();
    // let mut times = 1;
    // loop {
    //     if times >= 20 {
    //         break;
    //     }
    //     let task_id = Uuid::new_v4().to_string();
    //     let task_id2 = Uuid::new_v4().to_string();
    //     let req1 = Request::new(GetWorkerStatusRequest { task_id: task_id.clone() });
    //     let req2 = Request::new(GetWorkerStatusRequest { task_id: task_id2 });
    //     let unlock_req = Request::new(UnlockServerRequest { task_id });
    //     rt.block_on(async {
    //         match c.lock_server_if_free(req1).await {
    //             Ok(res) => {
    //                 println!("{}", res.into_inner().msg)
    //             }
    //             Err(s) => {
    //                 println!("{}", s.message())
    //             }
    //         }
    //
    //         match c.unlock_server(unlock_req).await {
    //             Ok(res) => {
    //                 println!("{}", res.into_inner().msg)
    //             }
    //             Err(s) => {
    //                 println!("{}", s.message())
    //             }
    //         }
    //
    //         match c.lock_server_if_free(req2).await {
    //             Ok(res) => {
    //                 println!("{}", res.into_inner().msg)
    //             }
    //             Err(s) => {
    //                 println!("{}", s.message())
    //             }
    //         }
    //     });
    //     times += 1;
    // }
    let task_id = Uuid::new_v4().to_string();
    let task_id2 = Uuid::new_v4().to_string();
    let task_id3 = Uuid::new_v4().to_string();
    let req1 = Request::new(GetWorkerStatusRequest { task_id: task_id.clone() });
    let req2 = Request::new(GetWorkerStatusRequest { task_id: task_id2 });
    let req3 = Request::new(GetWorkerStatusRequest { task_id: task_id3.clone() });
    let unlock_req1 = Request::new(UnlockServerRequest { task_id });
    let unlock_req2 = Request::new(UnlockServerRequest { task_id: task_id3 });
    rt.block_on(async {
        match c.lock_server_if_free(req1).await {
            Ok(res) => {
                println!("{}", res.into_inner().msg)
            }
            Err(s) => {
                println!("{}", s.message())
            }
        }

        match c.unlock_server(unlock_req1).await {
            Ok(res) => {
                println!("{}", res.into_inner().msg)
            }
            Err(s) => {
                println!("{}", s.message())
            }
        }

        match c.lock_server_if_free(req2).await {
            Ok(res) => {
                println!("{}", res.into_inner().msg)
            }
            Err(s) => {
                println!("{}", s.message())
            }
        }
        match c.lock_server_if_free(req3).await {
            Ok(res) => {
                println!("{}", res.into_inner().msg)
            }
            Err(s) => {
                println!("{}", s.message())
            }
        }
        match c.unlock_server(unlock_req2).await {
            Ok(res) => {
                println!("{}", res.into_inner().msg)
            }
            Err(s) => {
                println!("{}", s.message())
            }
        }
    });

    Ok(())
}