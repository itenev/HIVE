pub mod kernel;
pub mod identity;
pub mod hud;
pub mod observer;

use crate::models::scope::Scope;
use crate::memory::MemoryStore;
use std::sync::Arc;
use crate::prompts::hud::HudData;

pub struct SystemPromptBuilder;

impl SystemPromptBuilder {
    pub async fn assemble(scope: &Scope, memory_store: Arc<MemoryStore>) -> String {
        // Build live HUD data
        let hud_data = HudData::build(scope, memory_store).await;
        let hud_string = hud::format_hud(&hud_data);

        let kernel_string = kernel::get_laws();
        let identity_string = identity::get_persona();

        // Observer is NOT concatenated here; it runs as a separate 1:1 interceptor hook.
        // The core system prompt is just HUD + KERNEL + IDENTITY.
        format!("{}\n\n{}\n\n{}", hud_string, kernel_string, identity_string)
    }
}
