use adw::prelude::*;
use gettextrs::gettext;
use gtk::{gio, glib};
use relm4::{prelude::*, Component, ComponentParts, ComponentSender, Controller};
use serde_json::Value;

use crate::{
    components::{
        main_menu_button::MainMenuButton,
        message_thread::{MessageThreadInput, MessageThreadOutput, MessageThreadWidget},
        preferences::PreferencesModel,
    },
    config,
    workers::blueferry_worker::{
        blueferry_client::util::message_thread::MessageThread,
        worker::{BlueFerryWorker, BlueFerryWorkerInput, BlueFerryWorkerOutput},
    },
};

#[derive(Debug, Clone)]
pub struct Contact {
    pub name: String,
    pub address: String,
}

pub struct AppModel {
    worker: Controller<BlueFerryWorker>,
    threads: Vec<MessageThread>,
    contacts: Vec<Contact>,
    status: Option<Value>,
    selected: Option<usize>,
    thread_widget: Controller<MessageThreadWidget>,
    main_menu: Controller<MainMenuButton>,
    preferences: Option<Controller<PreferencesModel>>,
}

#[derive(Debug)]
pub enum AppModelInput {
    OpenPreferences,
    PreferencesClosed,
    Worker(BlueFerryWorkerOutput),
    NewMessage,
    ThreadSelected(u32),
    SendMessage { key: String, body: String },
    SendDirect { recipient: String, body: String },
    FindContacts { query: String },
    ShowError { error: String },
}

#[derive(Debug)]
pub enum AppModelOutput {}

#[relm4::component(pub)]
impl Component for AppModel {
    type Init = ();
    type Input = AppModelInput;
    type Output = AppModelOutput;
    type CommandOutput = ();

    view! {
        adw::ApplicationWindow {
            set_default_size: (1080, 800),
            set_title: Some(config::APPLICATION_NAME),

            #[wrap(Some)]
             #[name = "toast_overlay"]
            set_content = &adw::ToastOverlay {

                #[wrap(Some)]
                set_child = &adw::NavigationSplitView {
                    set_min_sidebar_width: 260.0,
                    set_max_sidebar_width: 340.0,

                    // -------------------- Sidebar --------------------
                    #[wrap(Some)]
                    set_sidebar = &adw::NavigationPage {
                        set_title: "Messages",
                        #[wrap(Some)]
                        set_child = &adw::ToolbarView {
                            add_top_bar = &adw::HeaderBar {
                                set_show_title: false,
                                pack_start = &gtk::ToggleButton {
                                    set_icon_name: "system-search-symbolic",
                                    set_tooltip_text: Some(&gettext("Search")),
                                },
                                pack_end = &gtk::Button {
                                    set_icon_name: "new-message-symbolic",
                                    set_tooltip_text: Some(&gettext("New Message")),
                                    connect_clicked[sender] => move |_| {
                                        sender.input(AppModelInput::NewMessage);
                                    }
                                }
                            },
                            #[wrap(Some)]
                            #[name = "sidebar"]
                            set_content = &adw::Sidebar {}
                        }
                    },

                    // -------------------- Conversation --------------------
                    #[wrap(Some)]
                    set_content = &adw::NavigationPage {
                        set_title: "Conversation",
                        #[wrap(Some)]
                        set_child = &adw::ToolbarView {
                            #[name = "header"]
                            add_top_bar = &adw::HeaderBar {
                                set_show_title: false,
                                pack_end = model.main_menu.widget() {}
                            },

                            // Error banner (hidden by default)
                            #[name = "error_banner"]
                            add_top_bar = &adw::Banner {
                                set_revealed: false,
                            },

                            #[wrap(Some)]
                            set_content = &gtk::Box {
                                set_orientation: gtk::Orientation::Vertical,
                                set_hexpand: true,
                                set_vexpand: true,
                                model.thread_widget.widget() {}
                            }
                        }
                    }
                }
            }
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let worker = BlueFerryWorker::builder()
            .launch(())
            .forward(sender.input_sender(), AppModelInput::Worker);
        worker.emit(BlueFerryWorkerInput::ListContacts {
            offset: 0,
            limit: 1000,
        });

        let thread_widget = MessageThreadWidget::builder().launch(()).forward(
            sender.input_sender(),
            |msg| match msg {
                MessageThreadOutput::SendMessage { key, body } => {
                    AppModelInput::SendMessage { key, body }
                }
            },
        );

        let main_menu = MainMenuButton::builder().launch(()).detach();

        let model = AppModel {
            worker,
            threads: Vec::new(),
            contacts: Vec::new(),
            status: None,
            selected: None,
            thread_widget,
            main_menu,
            preferences: None,
        };

        let widgets = view_output!();

        let sender_clone = sender.clone();
        widgets.sidebar.connect_activated(move |_sidebar, index| {
            sender_clone.input(AppModelInput::ThreadSelected(index));
        });

        // Register the preferences action here so its handler can reach the
        // app model (the controller is owned by the model).
        let prefs_action = gio::SimpleAction::new("preferences", None);
        let sender_clone = sender.clone();
        prefs_action.connect_activate(move |_, _| {
            sender_clone.input(AppModelInput::OpenPreferences);
        });
        relm4::main_application().add_action(&prefs_action);
        relm4::main_application().set_accels_for_action("app.preferences", &["<Primary>comma"]);

        ComponentParts { model, widgets }
    }

    fn update_with_view(
        &mut self,
        widgets: &mut Self::Widgets,
        message: Self::Input,
        sender: ComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match message {
            AppModelInput::Worker(out) => {
                self.handle_worker_output(out, sender, widgets);
                if let Some(selected) = self.selected {
                    widgets.sidebar.set_selected(selected as u32);
                }
            }

            AppModelInput::ThreadSelected(idx) => {
                let idx = idx as usize;
                self.selected = Some(idx);

                if let Some(thread) = self.threads.get(idx).cloned() {
                    self.thread_widget
                        .emit(MessageThreadInput::LoadThread(thread.clone()));

                    let label = gtk::Label::new(Some(&thread.name));
                    label.set_tooltip_text(Some(&format!(
                        "{} : {}",
                        gettext("Reply to"),
                        thread.reply_to_address()
                    )));

                    widgets.header.set_title_widget(Some(&label));
                    widgets.header.set_show_title(true);

                    self.worker.emit(BlueFerryWorkerInput::MarkRead {
                        key: thread.key.clone(),
                    });
                }
            }

            AppModelInput::SendDirect { recipient, body } => {
                self.worker
                    .emit(BlueFerryWorkerInput::SendDirect { recipient, body });
            }

            AppModelInput::SendMessage { key, body } => {
                self.worker.emit(BlueFerryWorkerInput::Send { key, body });
            }

            AppModelInput::FindContacts { query } => {
                // Contacts are cached at startup; the dialog filters locally.
                // Kept as an input so the dialog can still ask explicitly.
                self.worker
                    .emit(BlueFerryWorkerInput::FindContacts { query });
            }

            AppModelInput::NewMessage => {
                let contacts = self.contacts.clone();
                let sender = sender.clone();

                let app = relm4::main_application();
                let Some(window) = app.active_window() else {
                    return;
                };

                show_new_message_dialog(&window, contacts, move |recipient, body| {
                    sender.input(AppModelInput::SendDirect { recipient, body });
                });
            }
            AppModelInput::ShowError { error } => {
                let toast = adw::Toast::builder().title(&error).timeout(5).build();
                widgets.toast_overlay.add_toast(toast);
            }
            AppModelInput::OpenPreferences => {
                // If a dialog is already open, just present it again.
                if let Some(existing) = &self.preferences {
                    if let Some(window) = relm4::main_application().active_window() {
                        existing.widget().present(Some(&window));
                    }
                    return;
                }

                let controller = PreferencesModel::builder().launch(()).detach();
                let widget = controller.widget().clone();

                // When the dialog is closed, drop the controller so the next open
                // creates a fresh component (and a fresh connection worker).
                let sender_clone = sender.clone();
                widget.connect_closed(move |_| {
                    sender_clone.input(AppModelInput::PreferencesClosed);
                });

                if let Some(window) = relm4::main_application().active_window() {
                    widget.present(Some(&window));
                }

                self.preferences = Some(controller);
            }

            AppModelInput::PreferencesClosed => {
                self.preferences = None;
            }
        }
    }
}

impl AppModel {
    fn handle_worker_output(
        &mut self,
        out: BlueFerryWorkerOutput,
        sender: ComponentSender<Self>,
        widgets: &mut AppModelWidgets,
    ) {
        match out {
            BlueFerryWorkerOutput::Status(v) => {
                self.status = Some(v);
            }

            BlueFerryWorkerOutput::Threads(threads) => {
                self.threads = threads;
                self.rebuild_sidebar(widgets);

                // Refresh the open conversation so new messages appear.
                if let Some(idx) = self.selected {
                    if let Some(thread) = self.threads.get(idx).cloned() {
                        self.thread_widget
                            .emit(MessageThreadInput::LoadThread(thread));
                    }
                }
            }

            BlueFerryWorkerOutput::Sent { key, handle } => {
                eprintln!("sent {key} → {handle}");
            }

            BlueFerryWorkerOutput::Error { method, message } => {
                eprintln!("{method}: {message}");
                sender.input(AppModelInput::ShowError { error: message });
            }

            BlueFerryWorkerOutput::Contacts(value) => {
                self.contacts = parse_contacts(&value);
            }

            BlueFerryWorkerOutput::ContactSearch(value) => {
                eprintln!("contacts raw: {value}");
                self.contacts = parse_contacts(&value);
                eprintln!("parsed {} contacts", self.contacts.len())
            }

            BlueFerryWorkerOutput::Events(value) => {
                eprintln!("Events: {value}");
            }
            BlueFerryWorkerOutput::Recent(value) => {
                eprintln!("Recent: {value}");
            }
            BlueFerryWorkerOutput::MarkedRead { key, unread } => {
                eprintln!("Marked read: {key}: {unread}");
            }
            BlueFerryWorkerOutput::StarredChanged { key, starred } => {
                eprintln!("Starred changed: {key}: {starred}");
            }
            BlueFerryWorkerOutput::GroupParticipantsChanged(value) => {
                eprintln!("Group participants changed: {value}");
            }
            BlueFerryWorkerOutput::ThreadsDeleted(deleted) => {
                eprintln!("Threads deleted: {deleted}");
            }
            BlueFerryWorkerOutput::HistoryCleared => {
                eprintln!("History cleared");
            }
            BlueFerryWorkerOutput::NotificationPolicy(policy) => {
                eprintln!("Notification policy: {policy}");
            }
            BlueFerryWorkerOutput::ContactsOnlyNotifications(value) => {
                eprintln!("Contacts only notifications: {value}");
            }
            BlueFerryWorkerOutput::StoragePolicy(value) => {
                eprintln!("Storage policy: {value}");
            }
            BlueFerryWorkerOutput::Unlocked(value) => {
                eprintln!("Unlocked: {value}");
            }
            BlueFerryWorkerOutput::Healthy(value) => {
                eprintln!("Healthy: {value}");
            }
            BlueFerryWorkerOutput::LegacyOpened(value) => {
                eprintln!("Legacy opened: {value}");
            }
        }
    }

    fn rebuild_sidebar(&mut self, widgets: &mut AppModelWidgets) {
        let section = adw::SidebarSection::new();

        for thread in &self.threads {
            let mut builder = adw::SidebarItem::builder()
                .title(thread.display_name())
                .subtitle(thread.preview());

            let count = thread.unread_count();
            if count > 0 {
                let text = if count < 10 {
                    format!("0{count}")
                } else {
                    format!("{count}")
                };

                let badge = gtk::Label::new(Some(&text));
                badge.set_valign(gtk::Align::Center);
                badge.set_margin_start(6);
                badge.inline_css(
                    "min-width: 1.4em; \
                     padding: 1px 6px; \
                     border-radius: 9999px; \
                     background-color: @accent_bg_color; \
                     color: @accent_fg_color; \
                     font-weight: bold; \
                     font-size: 0.8em;",
                );

                builder = builder.suffix(&badge);
            }

            let item = builder.build();
            section.append(item);
        }

        widgets.sidebar.remove_all();
        widgets.sidebar.append(section);
    }
}

/// Parse the daemon's contact JSON: an array of
/// `{ "name": str, "phones": [str], "emails": [str] }`.
/// Flattens to one entry per address so the filter can match any of them.
fn parse_contacts(value: &Value) -> Vec<Contact> {
    let Some(arr) = value.as_array() else {
        return Vec::new();
    };

    let mut out = Vec::new();
    for item in arr {
        let Some(obj) = item.as_object() else {
            continue;
        };
        let name = obj
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        for field in ["phones", "emails"] {
            let Some(list) = obj.get(field).and_then(|v| v.as_array()) else {
                continue;
            };
            for addr in list {
                if let Some(a) = addr.as_str() {
                    if !a.is_empty() {
                        out.push(Contact {
                            name: name.clone(),
                            address: a.to_string(),
                        });
                    }
                }
            }
        }
    }
    out
}

/// Build and present the "New Message" dialog.
///
/// `on_send` receives the thread key (already prefixed) and the body.
fn show_new_message_dialog<F>(parent: &gtk::Window, contacts: Vec<Contact>, on_send: F)
where
    F: Fn(String, String) + 'static,
{
    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);

    let send_btn = gtk::Button::with_label(&gettext("Send"));
    send_btn.add_css_class("suggested-action");
    send_btn.set_sensitive(false);

    let header = adw::HeaderBar::builder()
        .show_end_title_buttons(false)
        .title_widget(&gtk::Label::new(Some(&gettext("New Message"))))
        .build();
    header.pack_end(&send_btn);
    root.append(&header);

    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content.set_margin_top(12);
    content.set_margin_bottom(16);
    content.set_margin_start(16);
    content.set_margin_end(16);

    // ---- Recipient row ----
    let recipient_entry = adw::EntryRow::builder().title(gettext("To")).build();
    recipient_entry.set_input_purpose(gtk::InputPurpose::Phone);

    let recipient_list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .css_classes(["boxed-list"])
        .build();
    recipient_list.append(&recipient_entry);
    content.append(&recipient_list);

    // ---- Contact suggestions ----
    let results_list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::Single)
        .css_classes(["boxed-list"])
        .margin_top(8)
        .build();

    let results_scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .min_content_height(160)
        .max_content_height(240)
        .propagate_natural_height(true)
        .build();
    results_scroll.set_child(Some(&results_list));
    results_scroll.set_visible(false);
    content.append(&results_scroll);

    // ---- Body ----
    let buffer = gtk::TextBuffer::new(None);
    let text_view = gtk::TextView::builder()
        .buffer(&buffer)
        .wrap_mode(gtk::WrapMode::WordChar)
        .top_margin(10)
        .bottom_margin(10)
        .left_margin(12)
        .right_margin(12)
        .build();

    let body_scroll = gtk::ScrolledWindow::builder()
        .hexpand(true)
        .vexpand(true)
        .min_content_height(140)
        .margin_top(12)
        .build();
    body_scroll.set_child(Some(&text_view));
    body_scroll.add_css_class("card");
    content.append(&body_scroll);

    root.append(&content);

    // ---- Filter contacts on each change ----
    {
        let results_list = results_list.clone();
        let results_scroll = results_scroll.clone();
        let contacts = contacts.clone();

        recipient_entry.connect_changed(move |entry| {
            while let Some(child) = results_list.first_child() {
                results_list.remove(&child);
            }

            let query = entry.text().trim().to_lowercase();
            if query.len() < 2 {
                results_scroll.set_visible(false);
                return;
            }

            let mut shown = 0usize;
            for c in contacts.iter() {
                if shown >= 25 {
                    break;
                }
                let name_hit = c.name.to_lowercase().contains(&query);
                let addr_hit = c.address.to_lowercase().contains(&query);
                if name_hit || addr_hit {
                    results_list.append(&build_contact_row(c));
                    shown += 1;
                }
            }

            results_scroll.set_visible(shown > 0);
        });
    }

    // ---- Suggestion activated: fill the entry ----
    {
        let recipient_entry = recipient_entry.clone();
        let results_scroll = results_scroll.clone();

        results_list.connect_row_activated(move |_, row| {
            let addr = row.widget_name();
            recipient_entry.set_text(&addr);
            results_scroll.set_visible(false);
        });
    }

    // ---- Send sensitivity ----
    {
        let send_btn = send_btn.clone();
        let recipient_entry_clone = recipient_entry.clone();
        let buffer_clone = buffer.clone();
        let recompute = move || {
            let has_recipient = !recipient_entry_clone.text().trim().is_empty();
            let (s, e) = (buffer_clone.start_iter(), buffer_clone.end_iter());
            let has_body = !buffer_clone.text(&s, &e, false).trim().is_empty();
            send_btn.set_sensitive(has_recipient && has_body);
        };

        let recompute_for_buffer = recompute.clone();
        recipient_entry.connect_changed(move |_| recompute());
        buffer.connect_changed(move |_| recompute_for_buffer());
    }

    let dialog = adw::Dialog::builder()
        .content_width(420)
        .content_height(440)
        .child(&root)
        .build();

    // ---- Send ----

    // ---- Send ----
    {
        let dialog = dialog.clone();
        let recipient_entry = recipient_entry.clone();
        let buffer = buffer.clone();

        send_btn.connect_clicked(move |_| {
            let recipient = recipient_entry.text().trim().to_string();
            let (s, e) = (buffer.start_iter(), buffer.end_iter());
            let body = buffer.text(&s, &e, false).to_string();

            if recipient.is_empty() || body.trim().is_empty() {
                return;
            }

            let raw = recipient
                .strip_prefix("address:phone:")
                .or_else(|| recipient.strip_prefix("address:email:"))
                .unwrap_or(&recipient)
                .to_string();

            on_send(raw, body);
            dialog.close();
        });
    }

    dialog.present(Some(parent));
}

fn build_contact_row(c: &Contact) -> gtk::ListBoxRow {
    let row_box = gtk::Box::new(gtk::Orientation::Vertical, 2);
    row_box.set_margin_all(8);

    let name = gtk::Label::builder()
        .label(if c.name.is_empty() {
            &c.address
        } else {
            &c.name
        })
        .xalign(0.0)
        .build();

    let addr = gtk::Label::builder()
        .label(&c.address)
        .xalign(0.0)
        .css_classes(["dim-label", "caption"])
        .build();

    row_box.append(&name);
    row_box.append(&addr);

    let row = gtk::ListBoxRow::new();
    row.set_child(Some(&row_box));
    row.set_widget_name(&c.address);
    row
}
