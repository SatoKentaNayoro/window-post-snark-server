use crate::error::{Error, Result};
use crate::snark_proof_grpc::snark_task_service_client::SnarkTaskServiceClient;
use std::time::Duration;
use tonic::transport::Channel;

pub async fn new_client(
    addr: &'static str,
    timeout: Duration,
) -> Result<SnarkTaskServiceClient<Channel>> {
    match Channel::from_shared(addr).timeout(timeout).connect().await {
        Ok(ch) => Ok(SnarkTaskServiceClient::new(ch)),
        Err(e) => Err(anyhow::Error::from(Error::NewClientFailed(e.to_string()))),
    }
}
