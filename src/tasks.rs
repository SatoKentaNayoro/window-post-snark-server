use crate::status::TaskStatus;

#[derive(Default, Debug, Clone)]
pub struct TaskInfo {
    pub task_id: String,
    pub vanilla_proof: Vec<u8>,
    pub pub_in: Vec<u8>,
    pub post_config: Vec<u8>,
    pub replicas_len: usize,
    pub result: Vec<u8>,
    pub task_status: TaskStatus,
}
