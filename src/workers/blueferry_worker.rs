use futures_util::StreamExt;
use relm4::prelude::*;
use relm4::tokio;
use relm4::Worker;
use serde_json::Value;
use zbus::Connection;

use crate::workers::blueferry_client::EventsProxy;
use crate::workers::blueferry_client::MessagesProxy;
use crate::workers::message_thread::Thread;

#[derive(Debug)]
pub enum WorkerInput {
    Refresh,
    Send { key: String, body: String },
    MarkRead { key: String },
}

#[derive(Debug)]
pub enum WorkerOutput {
    Status(Value),
    Threads(Vec<Thread>),
    Sent { key: String, handle: String },
    Error(String),
}

pub struct BlueFerryWorker {
    proxy: MessagesProxy<'static>,
}

impl Worker for BlueFerryWorker {
    type Init = ();
    type Input = WorkerInput;
    type Output = WorkerOutput;

    fn init(_init: (), sender: ComponentSender<Self>) -> Self {
        let (messages_proxy, events_proxy) = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let conn = Connection::session()
                    .await
                    .expect("session bus unavailable");

                let messages = MessagesProxy::new(&conn)
                    .await
                    .expect("Messages1 proxy unavailable");

                let events = EventsProxy::new(&conn)
                    .await
                    .expect("Events1 proxy unavailable");

                (messages, events)
            })
        });

        // 1. Listen to Messages1.message_received (legacy / optional)
        {
            let proxy = messages_proxy.clone();
            let sender = sender.clone();
            tokio::spawn(async move {
                match proxy.receive_message_received().await {
                    Ok(mut stream) => {
                        eprintln!("[BlueFerry] subscribed to Messages1.message_received");
                        while let Some(_) = stream.next().await {
                            eprintln!("[BlueFerry] Messages1.message_received → Refresh");
                            sender.input(WorkerInput::Refresh);
                        }
                    }
                    Err(e) => {
                        eprintln!("[BlueFerry] Messages1.message_received subscribe failed: {e}");
                    }
                }
            });
        }

        // 2. Listen to Events1.HistoryChanged  ← this is the important one
        {
            let proxy = events_proxy.clone();
            let sender = sender.clone();
            tokio::spawn(async move {
                match proxy.receive_history_changed().await {
                    Ok(mut stream) => {
                        eprintln!("[BlueFerry] subscribed to Events1.HistoryChanged");
                        while let Some(_) = stream.next().await {
                            eprintln!("[BlueFerry] ★ HistoryChanged → Refresh");
                            sender.input(WorkerInput::Refresh);
                        }
                    }
                    Err(e) => {
                        eprintln!("[BlueFerry] Events1.HistoryChanged subscribe failed: {e}");
                        let _ = sender.output(WorkerOutput::Error(format!(
                            "HistoryChanged subscribe failed: {e}"
                        )));
                    }
                }
            });
        }

        // 3. Also listen to StatusChanged (optional but useful)
        {
            let proxy = events_proxy;
            let sender = sender.clone();
            tokio::spawn(async move {
                if let Ok(mut stream) = proxy.receive_status_changed().await {
                    while let Some(_) = stream.next().await {
                        eprintln!("[BlueFerry] StatusChanged → Refresh");
                        sender.input(WorkerInput::Refresh);
                    }
                }
            });
        }

        // Initial load
        sender.input(WorkerInput::Refresh);

        Self {
            proxy: messages_proxy,
        }
    }

    fn update(&mut self, msg: WorkerInput, sender: ComponentSender<Self>) {
        let proxy = self.proxy.clone();
        let sender = sender.clone();

        match msg {
            WorkerInput::Refresh => {
                tokio::spawn(async move {
                    let status_fut = fetch_status(&proxy, &sender);
                    let threads_fut = fetch_threads(&proxy, &sender);
                    let _ = tokio::join!(status_fut, threads_fut);
                });
            }

            WorkerInput::Send { key, body } => {
                tokio::spawn(async move {
                    match proxy.send_to_thread_checked(&key, &body, false, "").await {
                        Ok(handle) => {
                            let _ = sender.output(WorkerOutput::Sent {
                                key: key.clone(),
                                handle,
                            });
                            // refresh so the sent message appears immediately
                            fetch_status(&proxy, &sender).await;
                            fetch_threads(&proxy, &sender).await;
                        }
                        Err(e) => {
                            let _ = sender.output(WorkerOutput::Error(format!("send failed: {e}")));
                        }
                    }
                });
            }

            WorkerInput::MarkRead { key } => {
                tokio::spawn(async move {
                    if let Err(e) = proxy.mark_thread_read(&key).await {
                        let _ =
                            sender.output(WorkerOutput::Error(format!("mark_read failed: {e}")));
                    }
                });
            }
        }
    }
}

async fn fetch_status(proxy: &MessagesProxy<'_>, sender: &ComponentSender<BlueFerryWorker>) {
    match proxy.get_status().await {
        Ok(raw) => match serde_json::from_str::<Value>(&raw) {
            Ok(v) => {
                let _ = sender.output(WorkerOutput::Status(v));
            }
            Err(e) => {
                let _ = sender.output(WorkerOutput::Error(format!("status parse: {e}")));
            }
        },
        Err(e) => {
            let _ = sender.output(WorkerOutput::Error(format!("get_status: {e}")));
        }
    }
}

async fn fetch_threads(proxy: &MessagesProxy<'_>, sender: &ComponentSender<BlueFerryWorker>) {
    match proxy.list_threads(1000).await {
        Ok(raw) => match serde_json::from_str::<Vec<Thread>>(&raw) {
            Ok(threads) => {
                let _ = sender.output(WorkerOutput::Threads(threads));
            }
            Err(e) => {
                let _ = sender.output(WorkerOutput::Error(format!("threads parse: {e}")));
            }
        },
        Err(e) => {
            let _ = sender.output(WorkerOutput::Error(format!("list_threads: {e}")));
        }
    }
}
