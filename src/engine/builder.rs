use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::models::capabilities::AgentCapabilities;
use crate::memory::MemoryStore;
use crate::platforms::Platform;
use crate::providers::Provider;
use crate::agent::AgentManager;
use crate::engine::core::Engine;
use crate::engine::drives;
use crate::engine::outreach;
use crate::engine::inbox;
use crate::teacher::Teacher;

pub struct EngineBuilder {
    platforms: HashMap<String, Box<dyn Platform>>,
    provider: Option<Arc<dyn Provider>>,
    capabilities: AgentCapabilities,
    memory: MemoryStore,
    agent: Option<Arc<AgentManager>>,
    project_root: String,
}

impl EngineBuilder {
    pub fn new() -> Self {
        let project_root = std::env::current_dir()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        Self {
            platforms: HashMap::new(),
            provider: None,
            capabilities: AgentCapabilities::default(),
            memory: MemoryStore::new(None),
            agent: None,
            project_root,
        }
    }

    pub fn with_platform(mut self, platform: Box<dyn Platform>) -> Self {
        self.platforms.insert(platform.name().to_string(), platform);
        self
    }

    pub fn with_capabilities(mut self, capabilities: AgentCapabilities) -> Self {
        self.capabilities = capabilities;
        self
    }

    pub fn with_provider(mut self, provider: Arc<dyn Provider>) -> Self {
        self.provider = Some(provider);
        self
    }

    #[cfg(test)]
    pub fn with_memory(mut self, mem: MemoryStore) -> Self {
        self.memory = mem;
        self
    }

    pub fn build(self) -> Result<Engine, &'static str> {
        let provider = self.provider.ok_or("Engine requires a Provider to be set")?;
        let (tx, rx) = mpsc::channel(100);
        
        let memory = Arc::new(self.memory);
        
        let drives = Arc::new(drives::DriveSystem::new(&self.project_root));
        let outreach_gate = Arc::new(outreach::OutreachGate::new(&self.project_root, provider.clone()));
        let inbox = Arc::new(inbox::InboxManager::new(&self.project_root));
        
        let agent = match self.agent {
            Some(s) => s,
            None => Arc::new(
                AgentManager::new(provider.clone(), memory.clone())
                    .with_outreach(drives.clone(), outreach_gate.clone(), inbox.clone())
            ),
        };

        Ok(Engine::new(
            Arc::new(self.platforms),
            provider.clone(),
            Arc::new(self.capabilities),
            memory,
            agent,
            Arc::new(Teacher::new(None)),
            drives,
            outreach_gate,
            inbox,
            Some(tx),
            rx,
        ))
    }
}
