//! BlueZ pairing agent. Hosts org.bluez.Agent1 and bridges to the worker.
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};
use zbus::interface;
use zbus::zvariant::ObjectPath;

/// Messages the agent sends to the worker when BlueZ invokes it.
#[derive(Debug)]
pub enum AgentEvent {
    ConfirmationRequested {
        device: String,
        passkey: u32,
        reply: oneshot::Sender<bool>,
    },
    DisplayPasskey {
        device: String,
        passkey: u32,
        entered: u16,
        reply: oneshot::Sender<()>,
    },
    Cancelled,
    Released,
}

pub struct PairingAgent {
    tx: mpsc::UnboundedSender<AgentEvent>,
}

impl PairingAgent {
    pub fn new(tx: mpsc::UnboundedSender<AgentEvent>) -> Self {
        Self { tx }
    }
}

#[interface(name = "org.bluez.Agent1")]
impl PairingAgent {
    async fn request_confirmation(
        &self,
        device: ObjectPath<'_>,
        passkey: u32,
    ) -> zbus::fdo::Result<()> {
        let (reply_tx, reply_rx) = oneshot::channel();
        let _ = self.tx.send(AgentEvent::ConfirmationRequested {
            device: device.as_str().to_string(),
            passkey,
            reply: reply_tx,
        });
        match reply_rx.await {
            Ok(true) => Ok(()),
            Ok(false) => Err(zbus::fdo::Error::Failed("passkey rejected".into())),
            Err(_) => Err(zbus::fdo::Error::Failed("agent channel closed".into())),
        }
    }

    async fn display_passkey(
        &self,
        device: ObjectPath<'_>,
        passkey: u32,
        entered: u16,
    ) -> zbus::fdo::Result<()> {
        let (reply_tx, reply_rx) = oneshot::channel();
        let _ = self.tx.send(AgentEvent::DisplayPasskey {
            device: device.as_str().to_string(),
            passkey,
            entered,
            reply: reply_tx,
        });
        let _ = reply_rx.await;
        Ok(())
    }

    async fn cancel(&self) -> zbus::fdo::Result<()> {
        let _ = self.tx.send(AgentEvent::Cancelled);
        Ok(())
    }

    async fn release(&self) -> zbus::fdo::Result<()> {
        let _ = self.tx.send(AgentEvent::Released);
        Ok(())
    }
}

/// Agent capabilities accepted by BlueZ. `DisplayYesNo` fits our flow:
/// the user sees a 6-digit passkey and confirms on both devices.
pub const AGENT_CAPABILITY: &str = "DisplayYesNo";
