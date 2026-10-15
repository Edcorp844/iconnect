//! Connection worker: owns the BlueZ client and pairing agent. Handles
//! discovery, pairing, unpairing, and exposes phone state (battery,
//! connection status) to the UI.

pub mod agent;
pub mod ancs_advertisment;
pub mod bluez_client;

use std::sync::{Arc, Mutex};

use agent::{AgentEvent, PairingAgent, AGENT_CAPABILITY};
use bluez_client::{BluetoothDevice, BluezClient, PhoneBattery};
use relm4::prelude::*;
use relm4::tokio;
use relm4::Worker;
use tokio::sync::{mpsc, oneshot};
use zbus::zvariant::ObjectPath;

use crate::workers::connection_worker::bluez_client::AGENT_MANAGER_IFACE;
use crate::workers::connection_worker::bluez_client::AGENT_MANAGER_PATH;
use crate::workers::connection_worker::bluez_client::AGENT_OBJECT_PATH;

#[derive(Debug)]
pub enum ConnectionInput {
    // Discovery
    StartScan { seconds: u32 },
    StopScan,
    RefreshDevices,

    // Pairing
    PairDevice { device_path: String },
    ConfirmPairing { accepted: bool },
    CancelPairing,

    // Unpair
    Unpair { device_path: String },

    // Phone state
    ReadBattery { device_path: String },
    ReadConnectionStatus { device_path: String },
    ConnectPhone { mac: String, adapter: String },

    // Internal: from the pairing agent
    Agent(AgentEvent),

    // Setup (CoD + ANCS advert)
    PrepareClassic { adapter: String },
    StartAncsAdvert { adapter: String },
    StopAncsAdvert { adapter: String },
}

#[derive(Debug)]
pub enum ConnectionOutput {
    Devices(Vec<BluetoothDevice>),
    Scanning(bool),

    // Pairing flow
    PasskeyRequested {
        device_name: String,
        passkey: String,
    },
    PasskeyUpdate {
        entered: u16,
    },
    PairingComplete {
        device_path: String,
    },
    PairingFailed {
        reason: String,
    },

    // Unpair
    Unpaired {
        device_path: String,
    },

    // Phone state
    Battery {
        device_path: String,
        battery: Option<PhoneBattery>,
    },
    ConnectionStatus {
        device_path: String,
        connected: bool,
        paired: bool,
        trusted: bool,
    },

    PhoneConfigured, // local.env written and daemon restarted
    PhoneConfigureFailed {
        reason: String,
    },

    ClassicPrepared,
    AncsAdvertStarted,
    AncsAdvertFailed {
        reason: String,
    },

    Error {
        method: &'static str,
        message: String,
    },
}

pub struct ConnectionWorker {
    client: BluezClient,
    adapter_path: Arc<Mutex<Option<String>>>,
    pending_confirmation: Option<oneshot::Sender<bool>>,
    agent_tx: mpsc::UnboundedSender<AgentEvent>,
}

impl Worker for ConnectionWorker {
    type Init = ();
    type Input = ConnectionInput;
    type Output = ConnectionOutput;

    fn init(_init: (), sender: ComponentSender<Self>) -> Self {
        let client = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(BluezClient::new())
                .expect("BlueZ system bus unavailable")
        });

        let (agent_tx, mut agent_rx) = mpsc::unbounded_channel();
        let agent = PairingAgent::new(agent_tx.clone());
        let conn = client.conn.clone();
        let sender_clone = sender.clone();

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let object_server = conn.object_server();
                object_server
                    .at(AGENT_OBJECT_PATH, agent)
                    .await
                    .expect("register Agent1 object");
                let agent_path = ObjectPath::try_from(AGENT_OBJECT_PATH).unwrap();
                conn.call_method(
                    Some("org.bluez"),
                    AGENT_MANAGER_PATH,
                    Some(AGENT_MANAGER_IFACE),
                    "RegisterAgent",
                    &(agent_path, AGENT_CAPABILITY),
                )
                .await
                .expect("RegisterAgent failed");
            })
        });

        tokio::spawn(async move {
            while let Some(event) = agent_rx.recv().await {
                sender_clone.input(ConnectionInput::Agent(event));
            }
        });

        sender.input(ConnectionInput::RefreshDevices);

        Self {
            client,
            adapter_path: Arc::new(Mutex::new(None)),
            pending_confirmation: None,
            agent_tx,
        }
    }

    fn update(&mut self, msg: ConnectionInput, sender: ComponentSender<Self>) {
        // Agent events run on the worker thread and need &mut self for
        // pending_confirmation. Handle them inline, never spawned.
        if let ConnectionInput::Agent(event) = msg {
            match event {
                AgentEvent::ConfirmationRequested {
                    device,
                    passkey,
                    reply,
                } => {
                    self.pending_confirmation = Some(reply);
                    let device_name = device.rsplit('/').next().unwrap_or(&device).to_string();
                    let _ = sender.output(ConnectionOutput::PasskeyRequested {
                        device_name,
                        passkey: format!("{passkey:06}"),
                    });
                }
                AgentEvent::DisplayPasskey { entered, reply, .. } => {
                    let _ = sender.output(ConnectionOutput::PasskeyUpdate { entered });
                    let _ = reply.send(());
                }
                AgentEvent::Cancelled => {
                    self.pending_confirmation = None;
                    let _ = sender.output(ConnectionOutput::PairingFailed {
                        reason: "Pairing cancelled".into(),
                    });
                }
                AgentEvent::Released => {
                    self.pending_confirmation = None;
                }
            }
            return;
        }

        // UI-driven ConfirmPairing needs &mut self too.
        if let ConnectionInput::ConfirmPairing { accepted } = msg {
            if let Some(reply) = self.pending_confirmation.take() {
                let _ = reply.send(accepted);
            } else {
                let _ = sender.output(ConnectionOutput::PairingFailed {
                    reason: "no pairing pending".into(),
                });
            }
            return;
        }

        let client = self.client.clone();
        let adapter_path = self.adapter_path.clone();
        let sender = sender.clone();

        tokio::spawn(async move {
            match msg {
                ConnectionInput::StartScan { seconds } => {
                    // Extract the cached path before any await; MutexGuard
                    // is !Send and cannot cross an await point.
                    let cached: Option<String> = adapter_path.lock().unwrap().clone();

                    let adapter = match cached {
                        Some(a) => a,
                        None => match client.primary_adapter().await {
                            Ok(a) => {
                                *adapter_path.lock().unwrap() = Some(a.path.clone());
                                a.path
                            }
                            Err(e) => {
                                err(&sender, "primary_adapter", e);
                                return;
                            }
                        },
                    };

                    let _ = sender.output(ConnectionOutput::Scanning(true));
                    if let Err(e) = client.start_discovery(&adapter).await {
                        err(&sender, "start_discovery", e);
                    }

                    let deadline =
                        std::time::Instant::now() + std::time::Duration::from_secs(seconds as u64);
                    while std::time::Instant::now() < deadline {
                        if let Ok(devices) = client.devices_on(&adapter).await {
                            let _ = sender.output(ConnectionOutput::Devices(devices));
                        }
                        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                    }

                    let _ = client.stop_discovery(&adapter).await;
                    let _ = sender.output(ConnectionOutput::Scanning(false));
                    if let Ok(devices) = client.devices_on(&adapter).await {
                        let _ = sender.output(ConnectionOutput::Devices(devices));
                    }
                }

                ConnectionInput::StopScan => {
                    let cached: Option<String> = adapter_path.lock().unwrap().clone();
                    if let Some(adapter) = cached {
                        let _ = client.stop_discovery(&adapter).await;
                    }
                    let _ = sender.output(ConnectionOutput::Scanning(false));
                }

                ConnectionInput::RefreshDevices => match client.primary_adapter().await {
                    Ok(a) => {
                        *adapter_path.lock().unwrap() = Some(a.path.clone());
                        match client.devices_on(&a.path).await {
                            Ok(devices) => {
                                let _ = sender.output(ConnectionOutput::Devices(devices));
                            }
                            Err(e) => err(&sender, "devices_on", e),
                        }
                    }
                    Err(e) => err(&sender, "primary_adapter", e),
                },

                ConnectionInput::PairDevice { device_path } => {
                    // Connect-first: iOS initiates the pairing transaction
                    // when a profile connect is refused on an unpaired device.
                    // The Agent receives the passkey.
                    match client.connect_device(&device_path).await {
                        Ok(()) => {
                            let _ =
                                sender.output(ConnectionOutput::PairingComplete { device_path });
                        }
                        Err(e) => err(&sender, "connect_device", e),
                    }
                }

                ConnectionInput::CancelPairing => {
                    // Real cancellation flows through AgentEvent::Cancelled,
                    // which is handled synchronously above.
                }

                ConnectionInput::Unpair { device_path } => {
                    let cached: Option<String> = adapter_path.lock().unwrap().clone();
                    let Some(adapter) = cached else {
                        err(
                            &sender,
                            "unpair",
                            anyhow::anyhow!("no adapter cached; refresh first"),
                        );
                        return;
                    };
                    match client.unpair(&adapter, &device_path).await {
                        Ok(()) => {
                            let _ = sender.output(ConnectionOutput::Unpaired { device_path });
                            if let Ok(devices) = client.devices_on(&adapter).await {
                                let _ = sender.output(ConnectionOutput::Devices(devices));
                            }
                        }
                        Err(e) => err(&sender, "unpair", e),
                    }
                }

                ConnectionInput::ReadBattery { device_path } => {
                    match client.phone_battery(&device_path).await {
                        Ok(battery) => {
                            let _ = sender.output(ConnectionOutput::Battery {
                                device_path,
                                battery,
                            });
                        }
                        Err(e) => err(&sender, "phone_battery", e),
                    }
                }

                ConnectionInput::ReadConnectionStatus { device_path } => {
                    let connected = client
                        .device_property(&device_path, "Connected")
                        .await
                        .ok()
                        .and_then(|v| bool::try_from(&v).ok())
                        .unwrap_or(false);
                    let paired = client
                        .device_property(&device_path, "Paired")
                        .await
                        .ok()
                        .and_then(|v| bool::try_from(&v).ok())
                        .unwrap_or(false);
                    let trusted = client
                        .device_property(&device_path, "Trusted")
                        .await
                        .ok()
                        .and_then(|v| bool::try_from(&v).ok())
                        .unwrap_or(false);
                    let _ = sender.output(ConnectionOutput::ConnectionStatus {
                        device_path,
                        connected,
                        paired,
                        trusted,
                    });
                }

                // Handled synchronously above.
                ConnectionInput::Agent(_) | ConnectionInput::ConfirmPairing { .. } => {
                    unreachable!()
                }

                ConnectionInput::ConnectPhone { mac, adapter } => {
                    // 1. Write local.env (through flatpak-spawn if sandboxed).
                    let env_content = format!(
                        "BLUEFERRY_MAC={}\nBLUEFERRY_ADAPTER={}\n",
                        mac.to_uppercase(),
                        adapter,
                    );

                    let write_result = tokio::process::Command::new("flatpak-spawn")
                        .args([
                            "--host",
                            "sh",
                            "-c",
                            &format!(
                                "mkdir -p $HOME/.config/blueferry && \
                 printf '%s' '{}' > $HOME/.config/blueferry/local.env && \
                 chmod 600 $HOME/.config/blueferry/local.env",
                                env_content.replace('\'', "'\\''"),
                            ),
                        ])
                        .status()
                        .await;

                    if write_result.map(|s| !s.success()).unwrap_or(true) {
                        let _ = sender.output(ConnectionOutput::PhoneConfigureFailed {
                            reason: "could not write ~/.config/blueferry/local.env on the host"
                                .into(),
                        });
                        return;
                    }

                    // 2. Restart the user service.
                    let restart_result = tokio::process::Command::new("flatpak-spawn")
                        .args([
                            "--host",
                            "systemctl",
                            "--user",
                            "restart",
                            "blueferry.service",
                        ])
                        .status()
                        .await;

                    if restart_result.map(|s| !s.success()).unwrap_or(true) {
                        let _ = sender.output(ConnectionOutput::PhoneConfigureFailed {
                            reason: "could not restart blueferry.service on the host".into(),
                        });
                        return;
                    }

                    // 3. Give the daemon a moment, then let the UI ask for status.
                    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                    let _ = sender.output(ConnectionOutput::PhoneConfigured);
                }
                ConnectionInput::PairDevice { device_path } => {
                    // 1. Make sure CoD is correct *before* pairing
                    let cached = adapter_path.lock().unwrap().clone();
                    if let Some(ref adapter) = cached {
                        if let Err(e) = client
                            .set_cod(adapter.trim_start_matches("/org/bluez/"))
                            .await
                        {
                            // Not fatal – log and continue
                            eprintln!("set_cod warning: {e:#}");
                        }
                    }

                    // 2. Classic connect / pair (your existing logic)
                    match client.connect_device(&device_path).await {
                        Ok(()) => {
                            let _ = sender.output(ConnectionOutput::PairingComplete {
                                device_path: device_path.clone(),
                            });

                            // 3. Now that the bond exists, start the ANCS solicitation
                            //    This is what makes the toggles appear on the iPhone.
                            if let Some(adapter) = cached {
                                // Give the Classic link a moment to settle
                                tokio::time::sleep(std::time::Duration::from_secs(2)).await;

                                match client.start_ancs_advert(&adapter).await {
                                    Ok(()) => {
                                        let _ = sender.output(ConnectionOutput::AncsAdvertStarted);
                                        eprintln!("ANCS solicitation advertisement started – toggles should appear on iPhone");
                                    }
                                    Err(e) => {
                                        let _ = sender.output(ConnectionOutput::AncsAdvertFailed {
                                            reason: format!("{e:#}"),
                                        });
                                    }
                                }
                            }
                        }
                        Err(e) => err(&sender, "connect_device", e),
                    }
                }
                ConnectionInput::PrepareClassic { adapter } => {
                    match client.set_cod(&adapter).await {
                        Ok(()) => {
                            let _ = sender.output(ConnectionOutput::ClassicPrepared);
                        }
                        Err(e) => err(&sender, "set_cod", e),
                    }
                }

                ConnectionInput::StartAncsAdvert { adapter } => {
                    match client.start_ancs_advert(&adapter).await {
                        Ok(()) => {
                            let _ = sender.output(ConnectionOutput::AncsAdvertStarted);
                        }
                        Err(e) => {
                            let _ = sender.output(ConnectionOutput::AncsAdvertFailed {
                                reason: format!("{e:#}"),
                            });
                        }
                    }
                }

                ConnectionInput::StopAncsAdvert { adapter } => {
                    let _ = client.stop_ancs_advert(&adapter).await;
                }
            }
        });
    }
}

fn err(sender: &ComponentSender<ConnectionWorker>, method: &'static str, e: anyhow::Error) {
    let _ = sender.output(ConnectionOutput::Error {
        method,
        message: format!("{e:#}"),
    });
}
