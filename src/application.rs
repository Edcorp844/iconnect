use adw::prelude::*;
use gtk::prelude::*;
use relm4::{prelude::*, Component, ComponentParts, ComponentSender, Controller};
use serde_json::Value;

use crate::{
    components::{
        main_menu_button::MainMenuButton,
        message_thread::{MessageThreadInput, MessageThreadOutput, MessageThreadWidget},
    },
    config,
    workers::{
        blueferry_worker::{BlueFerryWorker, WorkerInput, WorkerOutput},
        message_thread::Thread,
    },
};

pub struct AppModel {
    worker: Controller<BlueFerryWorker>,
    threads: Vec<Thread>,
    status: Option<Value>,
    selected: Option<usize>,
    thread_widget: Controller<MessageThreadWidget>,
    sidebar_section: adw::SidebarSection,
}

#[derive(Debug)]
pub enum AppModelInput {
    Worker(WorkerOutput),
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
                            pack_end = &gtk::Button {
                                set_icon_name: "view-refresh-symbolic",
                                connect_clicked[sender] => move |_| {
                                    let _ = &sender;
                                }
                            }
                        },
                        #[wrap(Some)]
                        #[name = "sidebar"]
                        set_content = &adw::Sidebar {}
                    }
                },

                #[wrap(Some)]
                set_content = &adw::NavigationPage {
                    set_title: "Conversation",
                    #[wrap(Some)]
                    set_child = &adw::ToolbarView {
                        #[name="header"]
                        add_top_bar = &adw::HeaderBar {},

                        #[wrap(Some)]
                        #[name = "chat_container"]
                        set_content = &gtk::Box {
                            set_orientation: gtk::Orientation::Vertical,
                            set_hexpand: true,
                            set_vexpand: true,
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
        let widgets = view_output!();

        let thread_widget =
            MessageThreadWidget::builder()
                .launch(())
                .forward(sender.input_sender(), move |msg| match msg {
                    MessageThreadOutput::SendMessage { key, body } => {
                        AppModelInput::SendMessage { key, body }
                    }
                });

        // Put initial widget in the container
        widgets.chat_container.append(thread_widget.widget());

        let worker = BlueFerryWorker::builder()
            .launch(())
            .forward(sender.input_sender(), AppModelInput::Worker);

        let sidebar_section = adw::SidebarSection::new();
        widgets.sidebar.append(sidebar_section.clone());

        let sender_clone = sender.clone();
        widgets.sidebar.connect_activated(move |_sidebar, index| {
            sender_clone.input(AppModelInput::ThreadSelected(index));
        });
        let main_menu = MainMenuButton::builder().launch(()).detach();

        widgets.header.pack_end(main_menu.widget());

        let model = AppModel {
            worker,
            threads: Vec::new(),
            status: None,
            selected: None,
            thread_widget,
            sidebar_section,
        };

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: AppModelInput, sender: ComponentSender<Self>, _root: &Self::Root) {
        match msg {
            AppModelInput::Worker(out) => self.handle_worker_output(out, sender),

            AppModelInput::ThreadSelected(idx) => {
                let idx = idx as usize;
                self.selected = Some(idx);

                if let Some(thread) = self.threads.get(idx).cloned() {
                    self.thread_widget
                        .emit(MessageThreadInput::LoadThread(thread.clone()));
                    self.worker.emit(WorkerInput::MarkRead { key: thread.key });
                }
            }
            AppModelInput::SendMessage { key, body } => {
                self.worker.emit(WorkerInput::Send { key, body });
            }
        }
    }
}

impl AppModel {
    fn handle_worker_output(&mut self, out: WorkerOutput, _sender: ComponentSender<Self>) {
        match out {
            WorkerOutput::Status(v) => {
                self.status = Some(v);
            }

            WorkerOutput::Threads(threads) => {
                self.threads = threads;
                self.rebuild_sidebar();

                // Auto-select first thread if nothing is selected
                if self.selected.is_none() && !self.threads.is_empty() {
                    self.selected = Some(0);
                }

                // ★ Update the currently open conversation with the newest messages
                if let Some(idx) = self.selected {
                    if let Some(thread) = self.threads.get(idx).cloned() {
                        self.thread_widget
                            .emit(MessageThreadInput::LoadThread(thread));
                    }
                }
            }

            WorkerOutput::Sent { key, handle } => {
                eprintln!("sent {key} → {handle}");
            }

            WorkerOutput::Error(msg) => {
                eprintln!("worker error: {msg}");
            }
        }
    }

    fn rebuild_sidebar(&self) {
        self.sidebar_section.remove_all();

        for thread in &self.threads {
            let item = adw::SidebarItem::builder()
                .title(thread.display_name())
                .subtitle(thread.preview())
                .build();
            self.sidebar_section.append(item);
        }
    }
}
