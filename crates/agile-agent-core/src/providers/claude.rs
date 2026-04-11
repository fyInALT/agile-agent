use std::sync::mpsc::Sender;

use anyhow::Result;

use crate::provider::ProviderEvent;

pub fn start(_prompt: String, event_tx: Sender<ProviderEvent>) -> Result<()> {
    let _ = event_tx.send(ProviderEvent::Error(
        "claude integration is not implemented yet".to_string(),
    ));
    let _ = event_tx.send(ProviderEvent::Finished);
    Ok(())
}
