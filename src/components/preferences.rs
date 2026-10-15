use adw::prelude::*;
use gettextrs::gettext;
use relm4::{prelude::*, ComponentParts, ComponentSender, Controller};
use serde_json::Value;

use crate::workers::{
    blueferry_worker::worker::{BlueFerryWorker, BlueFerryWorkerInput, BlueFerryWorkerOutput},
    connection_worker::{
        bluez_client::BluetoothDevice, ConnectionInput, ConnectionOutput, ConnectionWorker,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StatusLevel {
    Ok,
    Warn,
    Error,
}

#[derive(Debug)]
pub enum PreferencesInput {
    // From the two workers
    Connection(ConnectionOutput),
    Messages(BlueFerryWorkerOutput),

    // Connection actions
    StartPairing,
    CancelPairing,
    ConfirmPairing(bool),
    Unpair,
    RecheckConnection,

    // Notifications
    NotificationPolicyChanged(u32),
    ContactsOnlyChanged(bool),
    AncsActionsChanged(bool),

    // Media
    MediaControlChanged(bool),
    MprisPlayerChanged(bool),

    // Call History
    CallHistoryChanged(bool),
    MissedCallPopupsChanged(bool),

    // Away Lock
    ProximityLockChanged(bool),

    // Phone Calls
    CallsEnabledChanged(bool),

    // Local Data
    StoragePolicyChanged(u32),
}

pub struct PreferencesModel {
    // Workers owned by this component
    connection: Controller<ConnectionWorker>,
    messages: Controller<BlueFerryWorker>,

    // Paired device state
    paired_device: Option<BluetoothDevice>,
    battery_percent: Option<u8>,
    scanning: bool,
    passkey: Option<(String, String)>,
    pair_button_label: String,

    // Connection-details panel
    bg_service_state: StatusLevel,
    bg_service_detail: String,
    messages_state: StatusLevel,
    messages_detail: String,
    contacts_state: StatusLevel,
    contacts_detail: String,
    ancs_state: StatusLevel,
    ancs_detail: String,

    // Cached strings for `#[watch]` (temporaries cannot be borrowed)
    paired_title: String,
    paired_subtitle: String,
    battery_label: String,

    // Notification preferences (currently UI-only)
    notification_policy: u32,
    contacts_only: bool,
    ancs_actions: bool,
    show_ancs_actions: bool,

    media_control: bool,
    mpris_player: bool,
    show_media: bool,

    call_history: bool,
    missed_call_popups: bool,
    show_call_history: bool,

    proximity_lock: bool,
    show_proximity_lock: bool,
    proximity_grace: u32,

    calls_enabled: bool,
    show_calls: bool,

    storage_policy: u32,
    discovered_devices: Vec<BluetoothDevice>,
}

#[relm4::component(pub)]
impl Component for PreferencesModel {
    type Init = ();
    type Input = PreferencesInput;
    type Output = ();
    type CommandOutput = ();

    view! {
        #[root]
        adw::PreferencesDialog {
            set_title: &gettext("Preferences"),
            set_search_enabled: true,

            // ── Connection ──────────────────────────────────
            add = &adw::PreferencesPage {
                set_title: &gettext("Connection"),
                set_icon_name: Some("iconnect-bluetooth-symbolic"),

                add = &adw::PreferencesGroup {
                    set_title: &gettext("Paired iPhone"),

                    // ---- Connected state ----
                    adw::ActionRow {
                        #[watch]
                        set_visible: model.paired_device.is_some(),
                        #[watch]
                        set_title: &model.paired_title,
                        #[watch]
                        set_subtitle: &model.paired_subtitle,

                        add_suffix= &gtk::Image{
                             #[watch]
                            set_visible: model.battery_percent.is_some(),
                            set_icon_name: Some("iconnect-battery-level-symbolic"),
                        },

                        add_suffix = &gtk::Label {
                            #[watch]
                            set_visible: model.battery_percent.is_some(),
                            #[watch]
                            set_label: &model.battery_label,
                            add_css_class: "dim-label",
                            set_valign: gtk::Align::Center,
                            set_margin_end: 8,
                        },

                        add_suffix = &gtk::Button {
                            set_label: &gettext("Unpair"),
                            set_css_classes: &vec!["destructive-action", "flat"],
                            set_valign: gtk::Align::Center,
                            connect_clicked[sender] => move |_| {
                                sender.input(PreferencesInput::Unpair);
                            }
                        }
                    },

                    // ---- Not connected state ----
                    adw::ActionRow {
                        #[watch]
                        set_visible: model.paired_device.is_none(),
                        set_title: &gettext("No iPhone connected"),
                        set_subtitle: &gettext("Pair an iPhone to use BlueFerry"),

                        add_suffix = &gtk::Button {
                            #[watch]
                            set_label: &model.pair_button_label,
                            #[watch]
                            set_sensitive: !model.scanning,
                            add_css_class: "suggested-action",
                            set_valign: gtk::Align::Center,
                            connect_clicked[sender] => move |_| {
                                sender.input(PreferencesInput::StartPairing);
                            }
                        }
                    },
                },

                // ── Connection Details ───────────────────────
                add = &adw::PreferencesGroup {
                    set_title: &gettext("Connection Details"),

                    #[wrap(Some)]
                    set_header_suffix = &gtk::Button {
                        set_label: &gettext("Recheck"),
                        set_css_classes: &vec!["flat", "accent"],
                        connect_clicked[sender] => move |_| {
                            sender.input(PreferencesInput::RecheckConnection);
                        }
                    },

                    adw::ActionRow {
                        set_title: &gettext("Background Service"),
                        #[watch]
                        set_subtitle: &model.bg_service_detail,
                        add_suffix = &gtk::Image {
                            #[watch]
                            set_icon_name: Some(match model.bg_service_state {
                                StatusLevel::Ok => "object-select-symbolic",
                                StatusLevel::Warn => "dialog-warning-symbolic",
                                StatusLevel::Error => "dialog-error-symbolic",
                            }),
                            #[watch]
                            add_css_class: match model.bg_service_state {
                                StatusLevel::Ok => "success",
                                StatusLevel::Warn => "warning",
                                StatusLevel::Error => "error",
                            },
                            set_valign: gtk::Align::Center,
                        },
                    },

                    adw::ActionRow {
                        set_title: &gettext("Messages"),
                        #[watch]
                        set_subtitle: &model.messages_detail,
                        add_suffix = &gtk::Image {
                            #[watch]
                            set_icon_name: Some(match model.messages_state {
                                StatusLevel::Ok => "object-select-symbolic",
                                StatusLevel::Warn => "dialog-warning-symbolic",
                                StatusLevel::Error => "dialog-error-symbolic",
                            }),
                            #[watch]
                            add_css_class: match model.messages_state {
                                StatusLevel::Ok => "success",
                                StatusLevel::Warn => "warning",
                                StatusLevel::Error => "error",
                            },
                            set_valign: gtk::Align::Center,
                        },
                    },

                    adw::ActionRow {
                        set_title: &gettext("Contacts"),
                        #[watch]
                        set_subtitle: &model.contacts_detail,
                        add_suffix = &gtk::Image {
                            #[watch]
                            set_icon_name: Some(match model.contacts_state {
                                StatusLevel::Ok => "object-select-symbolic",
                                StatusLevel::Warn => "dialog-warning-symbolic",
                                StatusLevel::Error => "dialog-error-symbolic",
                            }),
                            #[watch]
                            add_css_class: match model.contacts_state  {
                                StatusLevel::Ok => "success",
                                StatusLevel::Warn => "warning",
                                StatusLevel::Error => "error",
                            },
                            set_valign: gtk::Align::Center,
                        },
                    },

                    adw::ActionRow {
                        set_title: &gettext("iPhone Notifications"),
                        #[watch]
                        set_subtitle: &model.ancs_detail,
                        add_suffix = &gtk::Image {
                            #[watch]
                            set_icon_name: Some(match model.ancs_state {
                                StatusLevel::Ok => "object-select-symbolic",
                                StatusLevel::Warn => "dialog-warning-symbolic",
                                StatusLevel::Error => "dialog-error-symbolic",
                            }),
                            #[watch]
                            add_css_class: match model.ancs_state {
                                StatusLevel::Ok => "success",
                                StatusLevel::Warn => "warning",
                                StatusLevel::Error => "error",
                            },
                            set_valign: gtk::Align::Center,
                        },
                    },
                },
            },

            // ── General ─────────────────────────────────────
            add = &adw::PreferencesPage {
                set_title: &gettext("General"),
                set_icon_name: Some("general-preferences-symbolic"),

                add = &adw::PreferencesGroup {
                    set_title: "Desktop Notifications",
                    set_description: Some("Choose which iPhone events create desktop popups."),

                    adw::ComboRow {
                        set_title: "Show Popups",
                        set_subtitle: "Messages only is the default",
                        set_model: Some(&gtk::StringList::new(&[
                            "All iPhone Notifications",
                            "Messages Only",
                            "None",
                        ])),
                        #[watch]
                        set_selected: model.notification_policy,
                        connect_selected_notify[sender] => move |row| {
                            sender.input(PreferencesInput::NotificationPolicyChanged(row.selected()));
                        }
                    },

                    adw::SwitchRow {
                        set_title: "Only Notify for Contacts",
                        set_subtitle: "Unknown senders remain available in message history",
                        #[watch]
                        set_active: model.contacts_only,
                        #[watch]
                        set_sensitive: model.notification_policy != 2,
                        connect_active_notify[sender] => move |row| {
                            sender.input(PreferencesInput::ContactsOnlyChanged(row.is_active()));
                        }
                    },

                    adw::SwitchRow {
                        set_title: "Show iPhone Action Buttons",
                        set_subtitle: "A click runs the action on the iPhone (answer/decline call, etc.)",
                        #[watch]
                        set_active: model.ancs_actions,
                        #[watch]
                        set_visible: model.show_ancs_actions,
                        connect_active_notify[sender] => move |row| {
                            sender.input(PreferencesInput::AncsActionsChanged(row.is_active()));
                        }
                    },
                },

                add = &adw::PreferencesGroup {
                    set_title: "Media Control",
                    #[watch]
                    set_visible: model.show_media,

                    adw::SwitchRow {
                        set_title: "Show and Control What the iPhone Is Playing",
                        set_subtitle: "Uses the Bluetooth LE link that also carries notifications.",
                        #[watch]
                        set_active: model.media_control,
                        connect_active_notify[sender] => move |row| {
                            sender.input(PreferencesInput::MediaControlChanged(row.is_active()));
                        }
                    },

                    adw::SwitchRow {
                        set_title: "Also Show It in the Desktop Media Controls (MPRIS)",
                        set_subtitle: "Every application in your session can then read the title, artist and album.",
                        #[watch]
                        set_active: model.mpris_player,
                        #[watch]
                        set_sensitive: model.media_control,
                        connect_active_notify[sender] => move |row| {
                            sender.input(PreferencesInput::MprisPlayerChanged(row.is_active()));
                        }
                    },
                },

                add = &adw::PreferencesGroup {
                    set_title: "Local Data",
                    set_description: Some("Protects message history and cached contacts."),

                    adw::ComboRow {
                        set_title: "Storage",
                        set_model: Some(&gtk::StringList::new(&[
                            "Encrypted with Desktop Keyring",
                            "Unencrypted Local Data",
                            "Do Not Retain Local Data",
                        ])),
                        #[watch]
                        set_selected: model.storage_policy,
                        connect_selected_notify[sender] => move |row| {
                            sender.input(PreferencesInput::StoragePolicyChanged(row.selected()));
                        }
                    },
                },
            },

            // ── Calls ───────────────────────────────────────
            add = &adw::PreferencesPage {
                set_title: "Calls",
                set_icon_name: Some("call-preferences-symbolic"),

                add = &adw::PreferencesGroup {
                    set_title: "Call History",
                    set_description: Some("Keeps the iPhone's recent calls under your local storage setting."),
                    #[watch]
                    set_visible: model.show_call_history,

                    adw::SwitchRow {
                        set_title: "Keep the iPhone's Recent Calls",
                        #[watch]
                        set_active: model.call_history,
                        connect_active_notify[sender] => move |row| {
                            sender.input(PreferencesInput::CallHistoryChanged(row.is_active()));
                        }
                    },

                    adw::SwitchRow {
                        set_title: "Notify Me About Missed Calls",
                        #[watch]
                        set_active: model.missed_call_popups,
                        #[watch]
                        set_sensitive: model.call_history,
                        connect_active_notify[sender] => move |row| {
                            sender.input(PreferencesInput::MissedCallPopupsChanged(row.is_active()));
                        }
                    },
                },

                add = &adw::PreferencesGroup {
                    set_title: "Phone Calls",
                    set_description: Some("Experimental. Calls can ring and be answered here. Emergency calls should still be made on the iPhone."),
                    #[watch]
                    set_visible: model.show_calls,

                    adw::SwitchRow {
                        set_title: "Enable Phone Calls Through This Computer",
                        #[watch]
                        set_active: model.calls_enabled,
                        connect_active_notify[sender] => move |row| {
                            sender.input(PreferencesInput::CallsEnabledChanged(row.is_active()));
                        }
                    },
                },
            },

            // ── Privacy ─────────────────────────────────────
            add = &adw::PreferencesPage {
                set_title: "Privacy",
                set_icon_name: Some("privacy-preferences-symbolic"),

                add = &adw::PreferencesGroup {
                    set_title: "Away Lock",
                    set_description: Some("A convenience, not a security feature. BlueFerry never unlocks the desktop."),
                    #[watch]
                    set_visible: model.show_proximity_lock,

                    adw::SwitchRow {
                        set_title: "Lock the Desktop When My iPhone Goes Away",
                        #[watch]
                        set_subtitle: &format!("Locks after {} seconds away", model.proximity_grace),
                        #[watch]
                        set_active: model.proximity_lock,
                        connect_active_notify[sender] => move |row| {
                            sender.input(PreferencesInput::ProximityLockChanged(row.is_active()));
                        }
                    },
                },
            },
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        // Launch both workers inside this component. Their whole lifetime is
        // tied to the preferences dialog: drop the dialog, drop the workers.
        let connection = ConnectionWorker::builder()
            .launch(())
            .forward(sender.input_sender(), PreferencesInput::Connection);

        let messages = BlueFerryWorker::builder()
            .launch(())
            .forward(sender.input_sender(), PreferencesInput::Messages);

        // Prime both: connection for device list, messages for the status panel.
        connection.emit(ConnectionInput::RefreshDevices);
        messages.emit(BlueFerryWorkerInput::RefreshStatus);

        let model = PreferencesModel {
            connection,
            messages,

            paired_device: None,
            battery_percent: None,
            scanning: false,
            passkey: None,
            pair_button_label: gettext("Pair").to_string(),

            bg_service_state: StatusLevel::Warn,
            bg_service_detail: gettext("Checking…").to_string(),
            messages_state: StatusLevel::Warn,
            messages_detail: gettext("Checking…").to_string(),
            contacts_state: StatusLevel::Warn,
            contacts_detail: gettext("Checking…").to_string(),
            ancs_state: StatusLevel::Warn,
            ancs_detail: gettext("Checking…").to_string(),

            paired_title: String::new(),
            paired_subtitle: String::new(),
            battery_label: String::new(),

            notification_policy: 1,
            contacts_only: false,
            ancs_actions: false,
            show_ancs_actions: true,

            media_control: false,
            mpris_player: false,
            show_media: true,

            call_history: false,
            missed_call_popups: true,
            show_call_history: true,

            proximity_lock: false,
            show_proximity_lock: true,
            proximity_grace: 60,

            calls_enabled: false,
            show_calls: true,

            storage_policy: 0,
            discovered_devices: Vec::new(),
        };

        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(
        &mut self,
        msg: PreferencesInput,
        _sender: ComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match msg {
            PreferencesInput::Connection(out) => self.handle_connection_output(out),
            PreferencesInput::Messages(out) => self.handle_messages_output(out),

            PreferencesInput::StartPairing => {
                self.scanning = true;
                self.pair_button_label = gettext("Scanning…").to_string();
                self.connection
                    .emit(ConnectionInput::StartScan { seconds: 15 });
            }

            PreferencesInput::CancelPairing => {
                self.passkey = None;
                self.scanning = false;
                self.pair_button_label = gettext("Pair").to_string();
                self.connection.emit(ConnectionInput::CancelPairing);
            }

            PreferencesInput::ConfirmPairing(accepted) => {
                self.passkey = None;
                self.connection
                    .emit(ConnectionInput::ConfirmPairing { accepted });
            }

            PreferencesInput::Unpair => {
                if let Some(device) = &self.paired_device {
                    self.connection.emit(ConnectionInput::Unpair {
                        device_path: device.path.clone(),
                    });
                }
            }

            PreferencesInput::RecheckConnection => {
                self.connection.emit(ConnectionInput::RefreshDevices);
                self.messages.emit(BlueFerryWorkerInput::RefreshStatus);
                if let Some(device) = &self.paired_device {
                    self.connection.emit(ConnectionInput::ReadBattery {
                        device_path: device.path.clone(),
                    });
                    self.connection.emit(ConnectionInput::ReadConnectionStatus {
                        device_path: device.path.clone(),
                    });
                }
            }

            PreferencesInput::NotificationPolicyChanged(v) => self.notification_policy = v,
            PreferencesInput::ContactsOnlyChanged(v) => self.contacts_only = v,
            PreferencesInput::AncsActionsChanged(v) => self.ancs_actions = v,
            PreferencesInput::MediaControlChanged(v) => self.media_control = v,
            PreferencesInput::MprisPlayerChanged(v) => self.mpris_player = v,
            PreferencesInput::CallHistoryChanged(v) => self.call_history = v,
            PreferencesInput::MissedCallPopupsChanged(v) => self.missed_call_popups = v,
            PreferencesInput::ProximityLockChanged(v) => self.proximity_lock = v,
            PreferencesInput::CallsEnabledChanged(v) => self.calls_enabled = v,
            PreferencesInput::StoragePolicyChanged(v) => self.storage_policy = v,
        }
    }
}

impl PreferencesModel {
    fn handle_connection_output(&mut self, out: ConnectionOutput) {
        match out {
            ConnectionOutput::Scanning(active) => {
                self.scanning = active;
                self.pair_button_label = if active {
                    gettext("Scanning…").to_string()
                } else {
                    gettext("Pair").to_string()
                };
            }

            ConnectionOutput::Devices(devices) => {
                // 1. Always update the paired device (the one we already know)
                let mut paired_candidates: Vec<BluetoothDevice> = devices
                    .iter()
                    .filter(|d| d.paired && (d.ancs_bonded() || d.likely_iphone()))
                    .cloned()
                    .collect();
                paired_candidates.sort_by_key(|d| !d.ancs_bonded());
                self.paired_device = paired_candidates.into_iter().next();

                if let Some(device) = &self.paired_device {
                    self.paired_title = device.display_name().to_string();
                    self.paired_subtitle = device.address.clone();
                    self.connection.emit(ConnectionInput::ReadBattery {
                        device_path: device.path.clone(),
                    });
                    self.connection.emit(ConnectionInput::ReadConnectionStatus {
                        device_path: device.path.clone(),
                    });
                } else {
                    self.paired_title.clear();
                    self.paired_subtitle.clear();
                    self.battery_percent = None;
                    self.battery_label.clear();
                }

                // 2. While scanning, also keep the unpaired devices so the user can pick one
                if self.scanning {
                    self.discovered_devices = devices
                        .into_iter()
                        .filter(|d| !d.paired && d.likely_iphone())
                        .collect();

                    // Optional: sort by signal strength / name
                    self.discovered_devices
                        .sort_by(|a, b| a.display_name().cmp(&b.display_name()));

                    // self.connection.emit(ConnectionInput::PairDevice {
                    //     device_path: selected.path.clone(),
                    // });
                } else {
                    // When scan stops we can clear the temporary list
                    // (or keep the last results – up to you)
                    self.discovered_devices.clear();
                }
            }

            ConnectionOutput::PasskeyRequested {
                device_name,
                passkey,
            } => {
                self.passkey = Some((device_name, passkey));
                // TODO: open the passkey dialog
            }

            ConnectionOutput::PasskeyUpdate { .. } => {}

            // ---------- Pairing finished ----------
            ConnectionOutput::PairingComplete { device_path } => {
                self.scanning = false;
                self.passkey = None;
                self.pair_button_label = gettext("Pair").to_string();

                // The worker already started the ANCS advert after the bond.
                // Now configure the daemon with the MAC + adapter.
                if let Some(device) = &self.paired_device {
                    // Prefer the real adapter name from the device path if possible
                    let adapter = device
                        .path
                        .split('/')
                        .find(|s| s.starts_with("hci"))
                        .unwrap_or("hci0")
                        .to_string();

                    self.connection.emit(ConnectionInput::ConnectPhone {
                        mac: device.address.clone(),
                        adapter,
                    });
                } else {
                    // Fallback: still try with the path we just paired
                    eprintln!("[pairing] complete but no paired_device in model yet");
                }
            }

            ConnectionOutput::PairingFailed { reason } => {
                self.scanning = false;
                self.passkey = None;
                self.pair_button_label = gettext("Pair").to_string();
                eprintln!("[pairing] failed: {reason}");
            }

            // ---------- New: CoD + ANCS advert results ----------
            ConnectionOutput::ClassicPrepared => {
                eprintln!("[setup] Class-of-Device set to A/V Hands-Free");
            }

            ConnectionOutput::AncsAdvertStarted => {
                eprintln!("[setup] ANCS solicitation advertisement is live");
                // Optional: show a small status message in the UI
                // self.setup_detail = gettext("Waiting for iPhone toggles…").into();
            }

            ConnectionOutput::AncsAdvertFailed { reason } => {
                eprintln!("[setup] ANCS advert failed: {reason}");
                // You can surface this to the user if you want
                // self.setup_detail = format!("Advert failed: {reason}");
            }

            // ---------- Unpair ----------
            ConnectionOutput::Unpaired { device_path } => {
                if self
                    .paired_device
                    .as_ref()
                    .is_some_and(|d| d.path == device_path)
                {
                    self.paired_device = None;
                    self.battery_percent = None;
                    self.paired_title.clear();
                    self.paired_subtitle.clear();
                    self.battery_label.clear();
                }
            }

            // ---------- Battery ----------
            ConnectionOutput::Battery {
                device_path,
                battery,
            } => {
                if self
                    .paired_device
                    .as_ref()
                    .is_some_and(|d| d.path == device_path)
                {
                    self.battery_percent = battery.map(|b| b.percent);
                    self.battery_label = battery
                        .map(|b| format!("{}%", b.percent))
                        .unwrap_or_default();
                }
            }

            // ---------- Post-pair daemon configuration ----------
            ConnectionOutput::PhoneConfigured => {
                self.connection.emit(ConnectionInput::RefreshDevices);
                self.messages.emit(BlueFerryWorkerInput::RefreshStatus);
            }

            ConnectionOutput::PhoneConfigureFailed { reason } => {
                eprintln!("[connection] post-pair setup failed: {reason}");
            }

            ConnectionOutput::ConnectionStatus { .. } => {}

            ConnectionOutput::Error { method, message } => {
                eprintln!("[connection] {method}: {message}");
            }
        }
    }

    fn handle_messages_output(&mut self, out: BlueFerryWorkerOutput) {
        match out {
            BlueFerryWorkerOutput::Status(v) => self.apply_status(&v),

            BlueFerryWorkerOutput::Error { method, message } => {
                eprintln!("[messages] {method}: {message}");
            }

            // Other outputs are not relevant to the preferences dialog.
            _ => {}
        }
    }

    fn apply_status(&mut self, status: &Value) {
        // Background service
        let conn_state = status
            .get("connectivity_state")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let conn_detail = status
            .get("connectivity_detail")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let retry = status
            .get("retry_delay_seconds")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        (self.bg_service_state, self.bg_service_detail) = match conn_state {
            "ready" => (StatusLevel::Ok, gettext("Running")),
            "connecting" | "retrying" => (
                StatusLevel::Warn,
                if retry > 0 {
                    format!("{conn_detail} — retrying in {retry}s")
                } else if conn_detail.is_empty() {
                    gettext("Connecting…")
                } else {
                    conn_detail
                },
            ),
            _ => (
                StatusLevel::Error,
                if conn_detail.is_empty() {
                    gettext("Unavailable")
                } else {
                    conn_detail
                },
            ),
        };

        // Messages (MAP)
        let map = status.get("map").and_then(|v| v.as_bool()).unwrap_or(false);
        (self.messages_state, self.messages_detail) = if map {
            (StatusLevel::Ok, gettext("Connected"))
        } else {
            (
                StatusLevel::Warn,
                gettext("Unavailable — Check the iPhone Settings Below"),
            )
        };

        // Contacts (PBAP)
        let pbap = status
            .get("pbap")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        (self.contacts_state, self.contacts_detail) = if pbap {
            (StatusLevel::Ok, gettext("Connected"))
        } else {
            (StatusLevel::Warn, gettext("Unavailable"))
        };

        // iPhone Notifications (ANCS)
        let ancs = status
            .get("ancs_subscribed")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let le = status.get("le").and_then(|v| v.as_bool()).unwrap_or(false);
        (self.ancs_state, self.ancs_detail) = if ancs {
            (StatusLevel::Ok, gettext("Connected"))
        } else if !le {
            (
                StatusLevel::Warn,
                gettext("Unavailable — Bluetooth LE is off"),
            )
        } else {
            (StatusLevel::Warn, gettext("Unavailable"))
        };
    }
}
