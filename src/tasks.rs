use filecoin_proofs::{PoStConfig, PoStType, SectorSize};
use crate::api_version::ApiVersion;
use crate::snark_proof_grpc::SnarkTaskRequestParams;
use crate::status::TaskStatus;
use serde::{Deserialize, Serialize};
use storage_proofs_core::api_version::ApiVersion;
use storage_proofs_core::error::Result;


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

fn get_post_config(post_config_u8: Vec<u8>) -> Result<PoStConfig> {
    let post_config_v = serde_json::from_slice(&post_config_u8)?;
    let post_config = serde_json::from_value::<PoStConfig>(post_config_v)?;
    Ok(post_config)
}
