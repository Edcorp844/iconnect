use futures_util::StreamExt;
use relm4::prelude::*;
use relm4::tokio;
use relm4::Worker;
use serde_json::Value;
use zbus::Connection;

use crate::workers::blueferry_worker::blueferry_client::client::{EventsProxy, MessagesProxy};
use crate::workers::blueferry_worker::blueferry_client::util::message_thread::MessageThread;

#[derive(Debug)]
pub enum BlueFerryWorkerInput {
    // ---- refresh / snapshot ----
    Refresh,
    RefreshStatus,
    RefreshThreads,
    RefreshContacts,
    RefreshEvents {
        kinds: Vec<String>,
        limit: u32,
    },
    RefreshRecent {
        folder: String,
        limit: u32,
    },

    // ---- messages ----
    Send {
        key: String,
        body: String,
    },
    SendToThread {
        key: String,
        body: String,
        confirm_group: bool,
    },
    SendToThreadChecked {
        key: String,
        body: String,
        confirm_group: bool,
        expected_group_token: String,
    },
    SendDirect {
        recipient: String,
        body: String,
    },

    // ---- thread ops ----
    MarkRead {
        key: String,
    },
    SetStarred {
        key: String,
        starred: bool,
    },
    SetGroupParticipants {
        key: String,
        recipients: Vec<String>,
    },
    DeleteThreads {
        keys: Vec<String>,
        flag: bool,
    },
    ClearHistory {
        flag: bool,
    },

    // ---- contacts ----
    FindContacts {
        query: String,
    },
    ListContacts {
        offset: u32,
        limit: u32,
    },
    SyncContacts,

    // ---- policies ----
    GetNotificationPolicy,
    SetNotificationPolicy {
        policy: String,
    },
    GetContactsOnlyNotifications,
    SetContactsOnlyNotifications {
        enabled: bool,
    },
    GetStoragePolicy,
    SetStoragePolicy {
        policy: String,
    },
    UnlockStorage,

    // ---- health ----
    IsHealthy,

    // ---- legacy ----
    OpenLegacyGtkMessage {
        a: String,
        b: String,
    },
}

#[derive(Debug)]
pub enum BlueFerryWorkerOutput {
    Status(Value),
    Threads(Vec<MessageThread>),
    Contacts(Value),
    ContactSearch(Value),
    Events(Value),
    Recent(Value),

    Sent {
        key: String,
        handle: String,
    },
    MarkedRead {
        key: String,
        unread: u32,
    },
    StarredChanged {
        key: String,
        starred: bool,
    },
    GroupParticipantsChanged(Value),
    ThreadsDeleted(u32),
    HistoryCleared,

    NotificationPolicy(String),
    ContactsOnlyNotifications(bool),
    StoragePolicy(Value),
    Unlocked(Value),

    Healthy(bool),
    LegacyOpened(bool),

    Error {
        method: &'static str,
        message: String,
    },
}

pub struct BlueFerryWorker {
    messages: MessagesProxy<'static>,
    events: EventsProxy<'static>,
}

impl Worker for BlueFerryWorker {
    type Init = ();
    type Input = BlueFerryWorkerInput;
    type Output = BlueFerryWorkerOutput;

    fn init(_init: (), sender: ComponentSender<Self>) -> Self {
        let (messages, events) = tokio::task::block_in_place(|| {
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

        // ---- signal subscriptions ----
        subscribe_history_changed(&events, &sender);
        subscribe_status_changed(&events, &sender);
        subscribe_open_message_requested(&events, &sender);

        // ---- initial snapshot ----
        sender.input(BlueFerryWorkerInput::Refresh);

        Self { messages, events }
    }

    fn update(&mut self, msg: BlueFerryWorkerInput, sender: ComponentSender<Self>) {
        let proxy = self.messages.clone();
        let sender = sender.clone();

        tokio::spawn(async move {
            match msg {
                // ---------------- refresh ----------------
                BlueFerryWorkerInput::Refresh => {
                    let (s, t) = tokio::join!(
                        fetch_status(&proxy, &sender),
                        fetch_threads(&proxy, &sender),
                    );
                    let _ = (s, t);
                }
                BlueFerryWorkerInput::RefreshStatus => {
                    fetch_status(&proxy, &sender).await;
                }
                BlueFerryWorkerInput::RefreshThreads => {
                    fetch_threads(&proxy, &sender).await;
                }
                BlueFerryWorkerInput::RefreshContacts => {
                    let _ = fetch_contacts(&proxy, &sender, 0, 500).await;
                }
                BlueFerryWorkerInput::RefreshEvents { kinds, limit } => {
                    let kind_refs: Vec<&str> = kinds.iter().map(|s| s.as_str()).collect();
                    match proxy.list_events(&kind_refs, limit).await {
                        Ok(raw) => {
                            emit_json(&sender, "list_events", &raw, BlueFerryWorkerOutput::Events)
                        }
                        Err(e) => err(&sender, "list_events", e),
                    }
                }
                BlueFerryWorkerInput::RefreshRecent { folder, limit } => {
                    match proxy.list_recent(&folder, limit).await {
                        Ok(raw) => {
                            emit_json(&sender, "list_recent", &raw, BlueFerryWorkerOutput::Recent)
                        }
                        Err(e) => err(&sender, "list_recent", e),
                    }
                }

                // ---------------- sending ----------------
                BlueFerryWorkerInput::Send { key, body } => {
                    match proxy.send_to_thread_checked(&key, &body, false, "").await {
                        Ok(handle) => {
                            let _ = sender.output(BlueFerryWorkerOutput::Sent {
                                key: key.clone(),
                                handle,
                            });
                            let _ = tokio::join!(
                                fetch_status(&proxy, &sender),
                                fetch_threads(&proxy, &sender),
                            );
                        }
                        Err(e) => err(&sender, "send", e),
                    }
                }
                BlueFerryWorkerInput::SendToThread {
                    key,
                    body,
                    confirm_group,
                } => match proxy.send_to_thread(&key, &body, confirm_group).await {
                    Ok(handle) => {
                        let _ = sender.output(BlueFerryWorkerOutput::Sent { key, handle });
                        fetch_threads(&proxy, &sender).await;
                    }
                    Err(e) => err(&sender, "send_to_thread", e),
                },
                BlueFerryWorkerInput::SendToThreadChecked {
                    key,
                    body,
                    confirm_group,
                    expected_group_token,
                } => {
                    match proxy
                        .send_to_thread_checked(&key, &body, confirm_group, &expected_group_token)
                        .await
                    {
                        Ok(handle) => {
                            let _ = sender.output(BlueFerryWorkerOutput::Sent { key, handle });
                            fetch_threads(&proxy, &sender).await;
                        }
                        Err(e) => err(&sender, "send_to_thread_checked", e),
                    }
                }
                BlueFerryWorkerInput::SendDirect { recipient, body } => {
                    match proxy.send(&recipient, &body).await {
                        Ok(handle) => {
                            let _ = sender.output(BlueFerryWorkerOutput::Sent {
                                key: recipient,
                                handle,
                            });
                            fetch_threads(&proxy, &sender).await;
                        }
                        Err(e) => err(&sender, "send", e),
                    }
                }

                // ---------------- thread ops ----------------
                BlueFerryWorkerInput::MarkRead { key } => {
                    match proxy.mark_thread_read(&key).await {
                        Ok(unread) => {
                            let _ =
                                sender.output(BlueFerryWorkerOutput::MarkedRead { key, unread });
                        }
                        Err(e) => err(&sender, "mark_thread_read", e),
                    }
                }
                BlueFerryWorkerInput::SetStarred { key, starred } => {
                    match proxy.set_thread_starred(&key, starred).await {
                        Ok(v) => {
                            let _ = sender
                                .output(BlueFerryWorkerOutput::StarredChanged { key, starred: v });
                        }
                        Err(e) => err(&sender, "set_thread_starred", e),
                    }
                }
                BlueFerryWorkerInput::SetGroupParticipants { key, recipients } => {
                    let refs: Vec<&str> = recipients.iter().map(|s| s.as_str()).collect();
                    match proxy.set_group_participants(&key, &refs).await {
                        Ok(raw) => emit_json(
                            &sender,
                            "set_group_participants",
                            &raw,
                            BlueFerryWorkerOutput::GroupParticipantsChanged,
                        ),
                        Err(e) => err(&sender, "set_group_participants", e),
                    }
                }
                BlueFerryWorkerInput::DeleteThreads { keys, flag } => {
                    let refs: Vec<&str> = keys.iter().map(|s| s.as_str()).collect();
                    match proxy.delete_threads(&refs, flag).await {
                        Ok(n) => {
                            let _ = sender.output(BlueFerryWorkerOutput::ThreadsDeleted(n));
                            fetch_threads(&proxy, &sender).await;
                        }
                        Err(e) => err(&sender, "delete_threads", e),
                    }
                }
                BlueFerryWorkerInput::ClearHistory { flag } => {
                    match proxy.clear_history(flag).await {
                        Ok(()) => {
                            let _ = sender.output(BlueFerryWorkerOutput::HistoryCleared);
                            fetch_threads(&proxy, &sender).await;
                        }
                        Err(e) => err(&sender, "clear_history", e),
                    }
                }

                // ---------------- contacts ----------------
                BlueFerryWorkerInput::FindContacts { query } => {
                    match proxy.find_contacts(&query).await {
                        Ok(raw) => emit_json(
                            &sender,
                            "find_contacts",
                            &raw,
                            BlueFerryWorkerOutput::ContactSearch,
                        ),
                        Err(e) => err(&sender, "find_contacts", e),
                    }
                }
                BlueFerryWorkerInput::ListContacts { offset, limit } => {
                    let _ = fetch_contacts(&proxy, &sender, offset, limit).await;
                }
                BlueFerryWorkerInput::SyncContacts => {
                    match proxy.sync_contacts().await {
                        Ok(n) => {
                            // No dedicated output; reuse Contacts fetch to refresh.
                            eprintln!("[BlueFerry] synced {n} contacts");
                            let _ = fetch_contacts(&proxy, &sender, 0, 500).await;
                        }
                        Err(e) => err(&sender, "sync_contacts", e),
                    }
                }

                // ---------------- policies ----------------
                BlueFerryWorkerInput::GetNotificationPolicy => {
                    match proxy.get_notification_policy().await {
                        Ok(p) => {
                            let _ = sender.output(BlueFerryWorkerOutput::NotificationPolicy(p));
                        }
                        Err(e) => err(&sender, "get_notification_policy", e),
                    }
                }
                BlueFerryWorkerInput::SetNotificationPolicy { policy } => {
                    match proxy.set_notification_policy(&policy).await {
                        Ok(p) => {
                            let _ = sender.output(BlueFerryWorkerOutput::NotificationPolicy(p));
                        }
                        Err(e) => err(&sender, "set_notification_policy", e),
                    }
                }
                BlueFerryWorkerInput::GetContactsOnlyNotifications => {
                    match proxy.get_contacts_only_notifications().await {
                        Ok(v) => {
                            let _ =
                                sender.output(BlueFerryWorkerOutput::ContactsOnlyNotifications(v));
                        }
                        Err(e) => err(&sender, "get_contacts_only_notifications", e),
                    }
                }
                BlueFerryWorkerInput::SetContactsOnlyNotifications { enabled } => {
                    match proxy.set_contacts_only_notifications(enabled).await {
                        Ok(v) => {
                            let _ =
                                sender.output(BlueFerryWorkerOutput::ContactsOnlyNotifications(v));
                        }
                        Err(e) => err(&sender, "set_contacts_only_notifications", e),
                    }
                }
                BlueFerryWorkerInput::GetStoragePolicy => match proxy.get_storage_policy().await {
                    Ok(p) => {
                        let _ = sender.output(BlueFerryWorkerOutput::StoragePolicy(
                            serde_json::Value::String(p),
                        ));
                    }
                    Err(e) => err(&sender, "get_storage_policy", e),
                },
                BlueFerryWorkerInput::SetStoragePolicy { policy } => {
                    match proxy.set_storage_policy(&policy).await {
                        Ok(raw) => emit_json(
                            &sender,
                            "set_storage_policy",
                            &raw,
                            BlueFerryWorkerOutput::StoragePolicy,
                        ),
                        Err(e) => err(&sender, "set_storage_policy", e),
                    }
                }
                BlueFerryWorkerInput::UnlockStorage => match proxy.unlock_storage().await {
                    Ok(raw) => emit_json(
                        &sender,
                        "unlock_storage",
                        &raw,
                        BlueFerryWorkerOutput::Unlocked,
                    ),
                    Err(e) => err(&sender, "unlock_storage", e),
                },

                // ---------------- health / legacy ----------------
                BlueFerryWorkerInput::IsHealthy => match proxy.is_healthy().await {
                    Ok(v) => {
                        let _ = sender.output(BlueFerryWorkerOutput::Healthy(v));
                    }
                    Err(e) => err(&sender, "is_healthy", e),
                },
                BlueFerryWorkerInput::OpenLegacyGtkMessage { a, b } => {
                    match proxy.open_legacy_gtk_message(&a, &b).await {
                        Ok(v) => {
                            let _ = sender.output(BlueFerryWorkerOutput::LegacyOpened(v));
                        }
                        Err(e) => err(&sender, "open_legacy_gtk_message", e),
                    }
                }
            }
        });
    }
}

fn err_str(
    sender: &ComponentSender<BlueFerryWorker>,
    method: &'static str,
    msg: impl Into<String>,
) {
    let _ = sender.output(BlueFerryWorkerOutput::Error {
        method,
        message: msg.into(),
    });
}

fn err(sender: &ComponentSender<BlueFerryWorker>, method: &'static str, e: zbus::Error) {
    err_str(sender, method, e.to_string());
}

fn emit<T>(sender: &ComponentSender<BlueFerryWorker>, value: Result<T, zbus::Error>)
where
    T: Into<BlueFerryWorkerOutput>,
{
    match value {
        Ok(v) => {
            let _ = sender.output(v.into());
        }
        Err(e) => err(sender, "unknown", e),
    }
}

fn emit_json<F>(sender: &ComponentSender<BlueFerryWorker>, method: &'static str, raw: &str, wrap: F)
where
    F: FnOnce(Value) -> BlueFerryWorkerOutput,
{
    match serde_json::from_str::<Value>(raw) {
        Ok(v) => {
            let _ = sender.output(wrap(v));
        }
        Err(e) => {
            let _ = sender.output(BlueFerryWorkerOutput::Error {
                method,
                message: format!("json parse: {e}"),
            });
        }
    }
}

async fn fetch_status(proxy: &MessagesProxy<'_>, sender: &ComponentSender<BlueFerryWorker>) {
    match proxy.get_status().await {
        Ok(raw) => match serde_json::from_str::<Value>(&raw) {
            Ok(v) => {
                let _ = sender.output(BlueFerryWorkerOutput::Status(v));
            }
            Err(e) => {
                let _ = sender.output(BlueFerryWorkerOutput::Error {
                    method: "get_status",
                    message: format!("json parse: {e}"),
                });
            }
        },
        Err(e) => err(sender, "get_status", e),
    }
}

async fn fetch_threads(proxy: &MessagesProxy<'_>, sender: &ComponentSender<BlueFerryWorker>) {
    match proxy.list_threads(1000).await {
        Ok(raw) => match serde_json::from_str::<Vec<MessageThread>>(&raw) {
            Ok(threads) => {
                let _ = sender.output(BlueFerryWorkerOutput::Threads(threads));
            }
            Err(e) => {
                let _ = sender.output(BlueFerryWorkerOutput::Error {
                    method: "list_threads",
                    message: format!("json parse: {e}"),
                });
            }
        },
        Err(e) => err(sender, "list_threads", e),
    }
}

async fn fetch_contacts(
    proxy: &MessagesProxy<'_>,
    sender: &ComponentSender<BlueFerryWorker>,
    offset: u32,
    limit: u32,
) {
    match proxy.list_contacts(offset, limit).await {
        Ok(raw) => emit_json(
            sender,
            "list_contacts",
            &raw,
            BlueFerryWorkerOutput::Contacts,
        ),
        Err(e) => err(sender, "list_contacts", e),
    }
}

// ---- signal subscriptions ----

fn subscribe_history_changed(
    events: &EventsProxy<'static>,
    sender: &ComponentSender<BlueFerryWorker>,
) {
    let proxy = events.clone();
    let sender = sender.clone();
    tokio::spawn(async move {
        match proxy.receive_history_changed().await {
            Ok(mut stream) => {
                while stream.next().await.is_some() {
                    sender.input(BlueFerryWorkerInput::Refresh);
                }
            }
            Err(e) => {
                let _ = sender.output(BlueFerryWorkerOutput::Error {
                    method: "HistoryChanged",
                    message: format!("subscribe failed: {e}"),
                });
            }
        }
    });
}

fn subscribe_status_changed(
    events: &EventsProxy<'static>,
    sender: &ComponentSender<BlueFerryWorker>,
) {
    let proxy = events.clone();
    let sender = sender.clone();
    tokio::spawn(async move {
        if let Ok(mut stream) = proxy.receive_status_changed().await {
            while stream.next().await.is_some() {
                sender.input(BlueFerryWorkerInput::RefreshStatus);
            }
        }
    });
}

fn subscribe_open_message_requested(
    events: &EventsProxy<'static>,
    sender: &ComponentSender<BlueFerryWorker>,
) {
    let proxy = events.clone();
    let sender = sender.clone();
    tokio::spawn(async move {
        if let Ok(mut stream) = proxy.receive_open_message_requested().await {
            while let Some(sig) = stream.next().await {
                if let Ok(args) = sig.args() {
                    eprintln!("[BlueFerry] OpenMessageRequested({})", args.key);
                    let _ = sender.output(BlueFerryWorkerOutput::Error {
                        method: "open_message_requested",
                        message: format!("key={}", args.key),
                    });
                }
            }
        }
    });
}
