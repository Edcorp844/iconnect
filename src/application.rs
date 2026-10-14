use adw::prelude::*;
use relm4::{prelude::*, Component, ComponentParts, ComponentSender, Controller};
use serde_json::Value;

use crate::{
    components::{
        main_menu_button::MainMenuButton,
        message_thread::{MessageThreadInput, MessageThreadOutput, MessageThreadWidget},
    },
    config,
    workers::blueferry_worker::{
        blueferry_client::util::message_thread::MessageThread,
        worker::{BlueFerryWorker, BlueFerryWorkerInput, BlueFerryWorkerOutput},
    },
};

pub struct AppModel {
    worker: Controller<BlueFerryWorker>,
    threads: Vec<MessageThread>,
    status: Option<Value>,
    selected: Option<usize>,
    thread_widget: Controller<MessageThreadWidget>,
    main_menu: Controller<MainMenuButton>,
}

#[derive(Debug)]
pub enum AppModelInput {
    Worker(BlueFerryWorkerOutput),
    ThreadSelected(u32),
    SendMessage { key: String, body: String },
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
            set_content = &adw::NavigationSplitView {
                set_min_sidebar_width: 260.0,
                set_max_sidebar_width: 340.0,

                #[wrap(Some)]
                set_sidebar = &adw::NavigationPage {
                    set_title: "Messages",
                    #[wrap(Some)]
                    set_child = &adw::ToolbarView {
                        add_top_bar = &adw::HeaderBar {
                            set_show_title: false,
                            pack_start=&gtk::ToggleButton{
                                set_icon_name: "system-search-symbolic"
                            },
                            pack_end = &gtk::Button {
                                set_icon_name: "new-message-symbolic",
                                connect_clicked[sender] => move |_| {
                                    let _ = &sender;
                                }
                            }
                        },
                        #[wrap(Some)]
                        #[name = "sidebar"]
                        set_content = &adw::Sidebar {

                        }
                    }
                },

                #[wrap(Some)]
                set_content = &adw::NavigationPage {
                    #[watch]
                    set_title: "Conversation",
                    #[wrap(Some)]
                    set_child = &adw::ToolbarView {
                        #[name="header"]
                        add_top_bar = &adw::HeaderBar {
                            pack_end=model.main_menu.widget(){}
                        },

                        #[wrap(Some)]
                        #[name = "chat_container"]
                        set_content = &gtk::Box {
                            set_orientation: gtk::Orientation::Vertical,
                            set_hexpand: true,
                            set_vexpand: true,

                            model.thread_widget.widget(){}
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

        let thread_widget =
            MessageThreadWidget::builder()
                .launch(())
                .forward(sender.input_sender(), move |msg| match msg {
                    MessageThreadOutput::SendMessage { key, body } => {
                        AppModelInput::SendMessage { key, body }
                    }
                });

        let main_menu = MainMenuButton::builder().launch(()).detach();

        let model = AppModel {
            worker,
            threads: Vec::new(),
            status: None,
            selected: None,
            thread_widget,
            main_menu,
        };

        let widgets = view_output!();

        let sender_clone = sender.clone();
        widgets.sidebar.connect_activated(move |_sidebar, index| {
            sender_clone.input(AppModelInput::ThreadSelected(index));
        });

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
                    widgets.header.set_title_widget(Some(&label));
                    self.worker
                        .emit(BlueFerryWorkerInput::MarkRead { key: thread.key });
                }
            }
            AppModelInput::SendMessage { key, body } => {
                self.worker.emit(BlueFerryWorkerInput::Send { key, body });
            }
        }
    }
}

impl AppModel {
    fn handle_worker_output(
        &mut self,
        out: BlueFerryWorkerOutput,
        _sender: ComponentSender<Self>,
        widgets: &mut AppModelWidgets,
    ) {
        match out {
            BlueFerryWorkerOutput::Status(v) => {
                self.status = Some(v);
            }

            BlueFerryWorkerOutput::MessageThreads(threads) => {
                self.threads = threads;
                self.rebuild_sidebar(widgets);

                // ★ Update the currently open conversation with the newest messages
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

            BlueFerryWorkerOutput::Error(msg) => {
                eprintln!("worker error: {msg}");
            }
        }
    }

    fn rebuild_sidebar(&mut self, widgets: &mut AppModelWidgets) {
        let sidebar_section = adw::SidebarSection::new();

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

                // Inline CSS – no external file needed
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
            sidebar_section.append(item);
        }
        widgets.sidebar.remove_all();
        widgets.sidebar.append(sidebar_section);
    }
}
