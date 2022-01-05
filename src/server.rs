use crate::error;
use crate::snark_proof_grpc::snark_task_service_server::{
    SnarkTaskService, SnarkTaskServiceServer,
};
use crate::snark_proof_grpc::{
    BaseResponse, GetTaskResultRequest, GetTaskResultResponse, GetWorkerStatusRequest,
    SnarkTaskRequestParams, UnlockServerRequest,
};
use crate::status::{ServerStatus, TaskStatus};
use crate::tasks;
use crate::tasks::{set_task_info, TaskInfo};
use futures::FutureExt;
use log::info;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::oneshot;
use tonic::transport::Server;
use tonic::{Request, Response, Status};

pub struct WindowPostSnarkServer {
    pub server_info: Arc<Mutex<ServerInfo>>,
    task_run_tx: UnboundedSender<String>,
}

#[derive(Debug)]
pub struct ServerInfo {
    pub task_info: tasks::TaskInfo,
    pub status: ServerStatus,
    pub last_update_time: Instant,
    pub error: String,
}

impl Default for ServerInfo {
    fn default() -> Self {
        ServerInfo {
            task_info: tasks::TaskInfo::default(),
            status: ServerStatus::default(),
            last_update_time: Instant::now(),
            error: String::default(),
        }
    }
}

impl WindowPostSnarkServer {
    pub fn default(task_run_tx: UnboundedSender<String>) -> Self {
        WindowPostSnarkServer {
            server_info: Arc::new(Mutex::new(ServerInfo::default())),
            task_run_tx,
        }
    }

    fn do_task(&self, task_params: &SnarkTaskRequestParams) -> Result<(), Status> {
        let mut si = match self.server_info.lock() {
            Ok(s) => s,
            Err(e) => {
                return Err(Status::aborted(e.to_string()));
            }
        };
        // Determine whether the request to execute the task came from the locked task
        let task_id = task_params.task_id.clone();
        if si.status == ServerStatus::Locked && si.task_info.task_id == task_id {
            // set task info
            let task_info = set_task_info(task_params);
            // set server info
            si.task_info = task_info;
            si.status = ServerStatus::Working;
            si.last_update_time = Instant::now();
            match self.task_run_tx.send("ok".to_string()) {
                Ok(_) => Ok(()),
                Err(s) => Err(Status::cancelled(s.0)),
            }
        } else {
            match si.status {
                ServerStatus::Locked => Err(Status::cancelled(
                    "server was locked by another task, can not be used now",
                )),
                ServerStatus::Free => Err(Status::cancelled(
                    "server should be locked until task is executed",
                )),
                ServerStatus::Working => Err(Status::cancelled(
                    "server is working on another task, can not be used now",
                )),
                ServerStatus::Unknown => {
                    Err(Status::cancelled("server is Unknown, can not be used now"))
                }
            }
        }
    }

    fn lock_server_if_free(&self, task_id: String) -> Result<ServerStatus, Status> {
        let mut si = match self.server_info.lock() {
            Ok(s) => s,
            Err(e) => return Err(Status::aborted(e.to_string())),
        };
        match si.status {
            ServerStatus::Free => {
                si.task_info = TaskInfo::default();
                // server will be locked by client with task_id here at first
                si.status = ServerStatus::Locked;
                si.task_info.task_id = task_id.clone();
                si.last_update_time = Instant::now();
                Ok(ServerStatus::Free)
            }
            ServerStatus::Locked => {
                // if locked too long and still not received task from miner, unlock it
                if Instant::now().duration_since(si.last_update_time) > SERVER_LOCK_TIME_OUT {
                    si.task_info = TaskInfo::default();
                    si.status = ServerStatus::Locked;
                    si.task_info.task_id = task_id.clone();
                    si.last_update_time = Instant::now();
                    Ok(ServerStatus::Free)
                } else {
                    Ok(ServerStatus::Locked)
                }
            }
            ServerStatus::Working => {
                // if miner do not get result back in 10min after task done or failed, drop task
                if (si.task_info.task_status == TaskStatus::Done
                    && Instant::now().duration_since(si.last_update_time)
                        >= SERVER_TASK_GET_BACK_TIME_OUT)
                    || (si.task_info.task_status == TaskStatus::Failed
                        && Instant::now().duration_since(si.last_update_time)
                            >= SERVER_TASK_GET_BACK_TIME_OUT)
                {
                    si.task_info = TaskInfo::default();
                    si.status = ServerStatus::Locked;
                    si.task_info.task_id = task_id.clone();
                    si.last_update_time = Instant::now();
                    Ok(ServerStatus::Free)
                } else {
                    Ok(ServerStatus::Working)
                }
            }
            ServerStatus::Unknown => Ok(ServerStatus::Unknown),
        }
    }

    fn get_task_result(&self, task_id: String) -> Result<Vec<u8>, Status> {
        let mut si = match self.server_info.lock() {
            Ok(s) => s,
            Err(e) => {
                return Err(Status::aborted(e.to_string()));
            }
        };

        if si.status == ServerStatus::Working {
            if task_id != si.task_info.task_id {
                Err(Status::invalid_argument(
                    anyhow::Error::from(error::Error::InvalidParameters(format!(
                        "current working task id is:{},but:{}",
                        si.task_info.task_id, task_id
                    )))
                    .to_string(),
                ))
            } else {
                if si.task_info.task_status == TaskStatus::Done {
                    si.status = ServerStatus::Free;
                    si.last_update_time = Instant::now();
                    si.task_info.task_status = TaskStatus::Returned;
                    Ok(si.task_info.result.clone())
                } else if si.task_info.task_status == TaskStatus::Failed {
                    si.status = ServerStatus::Free;
                    si.last_update_time = Instant::now();
                    Err(Status::aborted(
                        anyhow::Error::from(error::Error::TaskFailedWithError(si.error.clone()))
                            .to_string(),
                    ))
                } else {
                    Err(Status::aborted(
                        anyhow::Error::from(error::Error::TaskStillRunning).to_string(),
                    ))
                }
            }
        } else {
            Err(Status::cancelled(
                anyhow::Error::from(error::Error::NoTaskRunningOnSever).to_string(),
            ))
        }
    }

    fn unlock(&self, task_id: String) -> Result<(), Status> {
        let mut si = match self.server_info.lock() {
            Ok(s) => s,
            Err(e) => {
                return Err(Status::aborted(e.to_string()));
            }
        };
        if si.status == ServerStatus::Free {
            Err(Status::cancelled("server is already Free"))
        } else {
            if si.status == ServerStatus::Locked {
                if task_id == si.task_info.task_id {
                    si.status = ServerStatus::default();
                    si.task_info = TaskInfo::default();
                    si.last_update_time = Instant::now();
                    Ok(())
                } else {
                    Err(Status::invalid_argument(format!(
                        "can not be unlocked by another task ,which is locked by task_id:{},but {}",
                        si.task_info.task_id, task_id
                    )))
                }
            } else {
                Err(Status::cancelled(
                    "this operation just used to unlock a server in status Locked",
                ))
            }
        }
    }
}

const SERVER_LOCK_TIME_OUT: Duration = Duration::from_secs(10);
const SERVER_TASK_GET_BACK_TIME_OUT: Duration = Duration::from_secs(60);

#[tonic::async_trait]
impl SnarkTaskService for WindowPostSnarkServer {
    async fn do_snark_task(
        &self,
        request: Request<SnarkTaskRequestParams>,
    ) -> Result<Response<BaseResponse>, Status> {
        // get all params
        let params_all = request.get_ref();
        match self.do_task(params_all) {
            Ok(_) => Ok({
                Response::new(BaseResponse {
                    msg: "ok".to_string(),
                })
            }),
            Err(e) => Err(e),
        }
    }

    async fn lock_server_if_free(
        &self,
        request: Request<GetWorkerStatusRequest>,
    ) -> Result<Response<BaseResponse>, Status> {
        match self.lock_server_if_free(request.into_inner().task_id) {
            Ok(s) => Ok(Response::new(BaseResponse { msg: s.to_string() })),
            Err(e) => Err(e),
        }
    }

    async fn get_snark_task_result(
        &self,
        request: Request<GetTaskResultRequest>,
    ) -> Result<Response<GetTaskResultResponse>, Status> {
        match self.get_task_result(request.into_inner().task_id) {
            Ok(v) => Ok(Response::new(GetTaskResultResponse {
                msg: "ok".to_string(),
                result: v,
            })),
            Err(e) => Err(e),
        }
    }

    async fn unlock_server(
        &self,
        request: Request<UnlockServerRequest>,
    ) -> Result<Response<BaseResponse>, Status> {
        match self.unlock(request.into_inner().task_id) {
            Ok(_) => Ok(Response::new(BaseResponse {
                msg: "ok".to_string(),
            })),
            Err(e) => Err(e),
        }
    }
}

pub async fn run_server(
    srv_exit_rx: oneshot::Receiver<String>,
    srv: WindowPostSnarkServer,
    port: String,
) {
    let mut addr_s = "0.0.0.0:".to_string();
    addr_s += &port;
    let addr = addr_s.parse::<SocketAddr>().unwrap();
    info!("Server listening on {}", addr);
    Server::builder()
        .accept_http1(true)
        .add_service(SnarkTaskServiceServer::new(srv))
        .serve_with_shutdown(addr, srv_exit_rx.map(drop))
        .await
        .unwrap();
    info!("server stop listen")
}
