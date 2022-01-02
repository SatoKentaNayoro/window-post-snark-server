use std::str::FromStr;
use strum_macros::{Display, EnumString};

#[derive(Debug, PartialEq, Clone, EnumString, Display)]
pub enum ServerStatus {
    #[strum(to_string = "Unknown")]
    Unknown,
    #[strum(to_string = "Free")]
    Free,
    #[strum(to_string = "Working")]
    Working,
    #[strum(to_string = "Locked")]
    Locked,
}

impl Default for ServerStatus {
    fn default() -> Self {
        ServerStatus::Free
    }
}

#[derive(Debug, PartialEq, Clone, EnumString, Display)]
pub enum TaskStatus {
    #[strum(to_string = "None")]
    None,
    #[strum(to_string = "Ready")]
    Ready,
    #[strum(to_string = "Working")]
    Working,
    #[strum(to_string = "Done")]
    Done,
    #[strum(to_string = "Returned")]
    Returned,
    #[strum(to_string = "Failed")]
    Failed,
}

impl Default for TaskStatus {
    fn default() -> Self {
        TaskStatus::None
    }
}
