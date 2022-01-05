use std::any::Any;

pub use anyhow::Result;

use bellperson::SynthesisError;

/// Custom error types
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("unclassified error: {}", _0)]
    Unclassified(String),
    #[error("Invalid parameters file: {}", _0)]
    InvalidParameters(String),
    #[error("no task running on this server")]
    NoTaskRunningOnSever,
    #[error("Task is still running, not completed")]
    TaskStillRunning,
    #[error("task failed with error: {}", _0)]
    TaskFailedWithError(String),
}

impl From<Box<dyn Any + Send>> for Error {
    fn from(inner: Box<dyn Any + Send>) -> Error {
        Error::Unclassified(format!("{:?}", dbg!(inner)))
    }
}
