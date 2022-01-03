use crate::snark_proof_grpc::snark_task_service_server::SnarkTaskService;
use crate::snark_proof_grpc::{
    BaseResponse, GetTaskResultRequest, GetTaskResultResponse, GetWorkerStatusRequest,
    SnarkTaskRequestParams, UnlockServerRequest,
};
use crate::status::ServerStatus;
use crate::tasks;
use std::sync::{Arc, Mutex};
use std::time::Instant;
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
        todo!()
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
