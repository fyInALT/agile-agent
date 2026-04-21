#[cfg(feature = "core")]
pub mod app_runner;

#[cfg(not(feature = "core"))]
pub mod protocol_client;

#[cfg(not(feature = "core"))]
pub mod app_runner {
    use anyhow::Result;

    pub fn run() -> Result<()> {
        agent_tui::run_tui()
    }
}
