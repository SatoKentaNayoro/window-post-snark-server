use crate::status::{ServerStatus};
use crate::tasks;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::sync::mpsc::UnboundedSender;

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