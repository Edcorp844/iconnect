use adw::prelude::*;
use gettextrs::gettext;
use gtk::prelude::*;
use relm4::{prelude::*, ComponentParts, ComponentSender};

#[derive(Debug)]
pub enum PreferencesInput {
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

#[derive(Debug)]
pub enum PreferencesOutput {
    // You can forward these to your worker later
}

pub struct PreferencesModel {
    // Notifications
    notification_policy: u32, // 0=all, 1=messages, 2=none
    contacts_only: bool,
    ancs_actions: bool,
    show_ancs_actions: bool,

    // Media
    media_control: bool,
    mpris_player: bool,
    show_media: bool,

    // Call History
    call_history: bool,
    missed_call_popups: bool,
    show_call_history: bool,

    // Away Lock
    proximity_lock: bool,
    show_proximity_lock: bool,
    proximity_grace: u32,

    // Phone Calls
    calls_enabled: bool,
    show_calls: bool,

    // Local Data
    storage_policy: u32, // 0=encrypted, 1=plaintext, 2=none
}

#[relm4::component(pub)]
impl Component for PreferencesModel {
    type Init = ();
    type Input = PreferencesInput;
    type Output = PreferencesOutput;
    type CommandOutput = ();

    view! {
        #[root]
        adw::PreferencesDialog {
            set_title: &gettext("Preferences"),
            set_search_enabled: true,

            // ── Notifications ────────────────────────────────
            add = &adw::PreferencesPage {
                set_title: &gettext("Connection"),
                set_icon_name: Some("preferences-system-notifications-symbolic"),

                add = &adw::PreferencesGroup {
                    set_title: &gettext("Paired Iphone"),

                    adw::ActionRow {
                        set_title: "Ihone name",
                        set_subtitle: "Mac address",
                        add_suffix=&gtk::Button{
                            set_label: &gettext("Unpair"),
                            set_css_classes: &vec!["destructive-action", ],
                            set_valign: gtk::Align::Center,
                        }
                    },
                },

                add = &adw::PreferencesGroup {
                    set_title: &gettext("Connection Details"),

                    adw::ActionRow {
                        set_title: &gettext("Background Service"),
                        set_subtitle: "Status",
                        add_suffix=&gtk::Button{
                            set_label: &gettext("Unpair"),
                            set_css_classes: &vec!["destructive-action", ],
                            set_valign: gtk::Align::Center,
                        }
                    },
                    adw::ActionRow {
                        set_title: &gettext("Messages"),
                        set_subtitle: "Status",
                        add_suffix=&gtk::Button{
                            set_label: &gettext("Unpair"),
                            set_css_classes: &vec!["destructive-action", ],
                            set_valign: gtk::Align::Center,
                        }
                    },
                    adw::ActionRow {
                        set_title: &gettext("Contacts"),
                        set_subtitle: "Status",
                        add_suffix=&gtk::Button{
                            set_label: &gettext("Unpair"),
                            set_css_classes: &vec!["destructive-action", ],
                            set_valign: gtk::Align::Center,
                        }
                    },
                    adw::ActionRow {
                        set_title: &gettext("Iphone notications"),
                        set_subtitle: "Status",
                        add_suffix=&gtk::Button{
                            set_label: &gettext("Unpair"),
                            set_css_classes: &vec!["destructive-action", ],
                            set_valign: gtk::Align::Center,
                        }
                    },
                },


            },

            // ── Media ────────────────────────────────────────
            add = &adw::PreferencesPage {
                set_title: &gettext("General"),
                set_icon_name: Some("audio-headphones-symbolic"),

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

            // ── Calls ────────────────────────────────────────
            add = &adw::PreferencesPage {
                set_title: "Calls",
                set_icon_name: Some("call-start-symbolic"),

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

            // ── Privacy & Data ───────────────────────────────
            add = &adw::PreferencesPage {
                set_title: "Privacy",
                set_icon_name: Some("security-high-symbolic"),

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
        let model = PreferencesModel {
            notification_policy: 1, // messages only
            contacts_only: true,
            ancs_actions: true,
            show_ancs_actions: true,

            media_control: true,
            mpris_player: true,
            show_media: true,

            call_history: true,
            missed_call_popups: true,
            show_call_history: true,

            proximity_lock: true,
            show_proximity_lock: true,
            proximity_grace: 60,

            calls_enabled: true,
            show_calls: true,

            storage_policy: 0, // encrypted
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
