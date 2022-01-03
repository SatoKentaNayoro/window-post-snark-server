use crate::snark_proof_grpc::SnarkTaskRequestParams;
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

pub fn set_task_info(snark_params: &SnarkTaskRequestParams) -> TaskInfo {
    let task_info = TaskInfo {
        task_id: snark_params.task_id.clone(),
        vanilla_proof: snark_params.vanilla_proof.clone(),
        pub_in: snark_params.pub_in.clone(),
        post_config: snark_params.post_config.clone(),
        replicas_len: snark_params.replicas_len as usize,
        result: vec![],
        task_status: TaskStatus::Ready,
    };
    task_info
}
