use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DroneTemplate {
    pub name: String,
    pub system_prompt: String,
    pub tools: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DroneStatus {
    Success,
    Failed(String),
    Timeout,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DroneResult {
    pub task_id: String,
    pub output: String,
    pub tokens_used: u32,
    pub status: DroneStatus,
}
