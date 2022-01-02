use crate::status;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::sync::mpsc::UnboundedSender;

pub struct WindowPostServer {
    pub server_info: Arc<Mutex<ServerInfo>>,
    task_run_tx: UnboundedSender<String>,
}

#[derive(Debug)]
pub struct ServerInfo {
    // pub task_info: TaskInfo,
    pub status: status::ServerStatus,
    pub last_update_time: Instant,
    pub error: String,
}
