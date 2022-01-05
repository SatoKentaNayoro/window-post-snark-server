use crate::server::ServerInfo;
use crate::snark_proof_grpc::SnarkTaskRequestParams;
use crate::status::TaskStatus;
use filecoin_proofs::caches::get_post_params;
use filecoin_proofs::parameters::window_post_setup_params;
use filecoin_proofs::{get_partitions_for_window_post, with_shape, PoStConfig};
use log::{error, info, warn};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use storage_proofs_core::{
    compound_proof, compound_proof::CompoundProof, error::Result, merkle::MerkleTreeTrait,
};
use storage_proofs_post::fallback::{FallbackPoSt, FallbackPoStCompound};
use tokio::select;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::oneshot;

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

fn get_post_config(post_config_u8: &Vec<u8>) -> Result<PoStConfig> {
    let post_config_v = serde_json::from_slice(post_config_u8)?;
    let post_config = serde_json::from_value::<PoStConfig>(post_config_v)?;
    Ok(post_config)
}

pub async fn run_task(
    exit_rx: oneshot::Receiver<String>,
    mut do_task_signal_rx: UnboundedReceiver<String>,
    srv_info: Arc<Mutex<ServerInfo>>,
) {
    info!("task worker run");
    let mission = async {
        loop {
            match do_task_signal_rx.recv().await {
                Some(value) => {
                    if value == "ok".to_string() {
                        let si1 = match srv_info.lock() {
                            Ok(s) => s,
                            Err(e) => {
                                error!("get lock failed with error: {}", e);
                                continue;
                            }
                        };

                        info!("start to do task: {}", si1.task_info.task_id);
                        let t = si1.task_info.clone();

                        let post_config = get_post_config(&t.post_config);
                        drop(si1);
                        // run snark
                        match post_config {
                            Ok(p) => {
                                let size = p.sector_size;
                                let result = with_shape!(size.0, run_snark, t);

                                let mut si2 = match srv_info.lock() {
                                    Ok(s) => s,
                                    Err(e) => {
                                        error!("get lock failed with error: {}", e);
                                        continue;
                                    }
                                };

                                match result {
                                    Ok(r) => {
                                        info!("task {} done", si2.task_info.task_id);
                                        si2.task_info.result = r;
                                        si2.task_info.task_status = TaskStatus::Done;
                                    }
                                    Err(e) => {
                                        error!(
                                            "snark task {} failed with error: {}",
                                            si2.task_info.task_id, e
                                        );
                                        si2.task_info.task_status = TaskStatus::Failed;
                                        si2.error = e.to_string();
                                    }
                                }
                                drop(si2)
                            }
                            Err(e) => {
                                error!("parse post config with error:{}", e);
                            }
                        }
                    } else {
                        error!("wrong signal {:?}", value);
                    }
                }
                None => (),
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    };

    let is_exit_signal;
    select! {
        _ = exit_rx => {
            info!("worker received an exit command,will exit after current task done");
            is_exit_signal = true;
            ()
        }
        _ = mission => {
            is_exit_signal = false;
            error!("task failed unexpected");
            ()
        }
    }
    if is_exit_signal {
        let exit_start_time = Instant::now();
        let (mut is_working_logged, mut is_done_logged) = (false, false);
        loop {
            let si = match srv_info.lock() {
                Ok(s) => s,
                Err(e) => {
                    error!("{}", e);
                    continue;
                }
            };
            match si.task_info.task_status {
                TaskStatus::None => {
                    info!("no task running, will exit immediately");
                    break;
                }
                TaskStatus::Ready => {
                    info!("task is ready but not start running, will exit immediately");
                    break;
                }
                TaskStatus::Working => {
                    if !is_working_logged {
                        is_working_logged = true;
                        info!("task is running,will exit after task done and result returned");
                    }
                    continue;
                }
                TaskStatus::Done => {
                    if Instant::now().duration_since(exit_start_time) > Duration::from_secs(300) {
                        warn!("worker has wait 5minute,force exited");
                        break;
                    } else {
                        if !is_done_logged {
                            is_done_logged = true;
                            info!("task is done,waiting for miner to get result back");
                        }
                        continue;
                    }
                }
                TaskStatus::Returned => {
                    info!("task result was returned,will exit immediately");
                    break;
                }
                TaskStatus::Failed => break,
            };
        }
    }
    info!("task worker exited");
}

fn run_snark<Tree: 'static + MerkleTreeTrait>(task_info: TaskInfo) -> Result<Vec<u8>> {
    let post_config_v = serde_json::from_slice(&task_info.post_config)?;
    let post_config = serde_json::from_value::<PoStConfig>(post_config_v)?;

    let vanilla_params = window_post_setup_params(&post_config);
    let partitions = get_partitions_for_window_post(task_info.replicas_len as usize, &post_config);
    let setup_params = compound_proof::SetupParams {
        vanilla_params,
        partitions,
        priority: post_config.priority,
    };
    let pub_params: compound_proof::PublicParams<'_, FallbackPoSt<'_, Tree>> =
        FallbackPoStCompound::setup(&setup_params)?;
    let vanilla_v = serde_json::from_slice(&task_info.vanilla_proof)?;
    let pub_in_v = serde_json::from_slice(&task_info.pub_in)?;
    let groth_params = get_post_params::<Tree>(&post_config)?;
    let proof = FallbackPoStCompound::prove_with_vanilla_by_snark_server(
        &pub_params,
        pub_in_v,
        vanilla_v,
        &groth_params,
    )?;
    proof.to_vec()
}
