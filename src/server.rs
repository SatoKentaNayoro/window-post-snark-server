use crate::snark_proof_grpc::snark_task_service_server::SnarkTaskService;
use crate::snark_proof_grpc::{
    BaseResponse, GetTaskResultRequest, GetTaskResultResponse, GetWorkerStatusRequest,
    SnarkTaskRequestParams, UnlockServerRequest,
};
use crate::status::{ServerStatus, TaskStatus};
use crate::tasks;
use crate::tasks::TaskInfo;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::mpsc::UnboundedSender;
use tonic::{Request, Response, Status};

pub struct WindowPostServer {
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

impl WindowPostServer {
    pub fn default(task_run_tx: UnboundedSender<String>) -> Self {
        WindowPostServer {
            server_info: Arc::new(Mutex::new(ServerInfo::default())),
            task_run_tx,
        }
    }
}

const SERVER_LOCK_TIME_OUT: Duration = Duration::from_secs(10);
const SERVER_TASK_GET_BACK_TIME_OUT: Duration = Duration::from_secs(60);

#[tonic::async_trait]
impl SnarkTaskService for WindowPostServer {
    async fn do_snark_task(
        &self,
        request: Request<SnarkTaskRequestParams>,
    ) -> Result<Response<BaseResponse>, Status> {
        todo!()
    }

    async fn lock_server_if_free(
        &self,
        request: Request<GetWorkerStatusRequest>,
    ) -> Result<Response<BaseResponse>, Status> {
        let mut si = match self.server_info.lock() {
            Ok(s) => s,
            Err(e) => return Err(Status::aborted(e.to_string())),
        };
        let ref req = request.into_inner();
        match si.status {
            ServerStatus::Free => {
                si.task_info = TaskInfo::default();
                // server will be locked by client with task_id here at first
                si.status = ServerStatus::Locked;
                si.task_info.task_id = req.task_id.clone();
                si.last_update_time = Instant::now();
                Ok(Response::new(BaseResponse {
                    msg: ServerStatus::Free.to_string(),
                }))
            }
            ServerStatus::Locked => {
                // if locked too long and still not received task from miner, unlock it
                if Instant::now().duration_since(si.last_update_time) > SERVER_LOCK_TIME_OUT {
                    si.task_info = TaskInfo::default();
                    si.status = ServerStatus::Locked;
                    si.task_info.task_id = req.task_id.clone();
                    si.last_update_time = Instant::now();
                    Ok(Response::new(BaseResponse {
                        msg: ServerStatus::Free.to_string(),
                    }))
                } else {
                    Ok(Response::new(BaseResponse {
                        msg: ServerStatus::Locked.to_string(),
                    }))
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
                    si.task_info.task_id = req.task_id.clone();
                    si.last_update_time = Instant::now();
                    Ok(Response::new(BaseResponse {
                        msg: ServerStatus::Free.to_string(),
                    }))
                } else {
                    Ok(Response::new(BaseResponse {
                        msg: ServerStatus::Working.to_string(),
                    }))
                }
            }
            ServerStatus::Unknown => Ok(Response::new(BaseResponse {
                msg: ServerStatus::Unknown.to_string(),
            })),
        }
    }

    async fn get_snark_task_result(
        &self,
        request: Request<GetTaskResultRequest>,
    ) -> Result<Response<GetTaskResultResponse>, Status> {
        todo!()
    }

    async fn unlock_server(
        &self,
        request: Request<UnlockServerRequest>,
    ) -> Result<Response<BaseResponse>, Status> {
        todo!()
    }
}
