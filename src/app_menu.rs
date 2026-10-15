use adw::prelude::*;
use gettextrs::gettext;
use gtk::{gio::prelude::*, prelude::*, Application};
use relm4::Component;

use crate::config;

pub struct AppMenu {
    app: Application,
}

impl AppMenu {
    pub fn new() -> Self {
        let app = relm4::main_application();
        Self { app }
    }

    pub fn register(&self) {
        let refresh_action = gtk::gio::SimpleAction::new("refresh", None);
        refresh_action.connect_activate(move |_, _| {
            println!("Refresh requested");
        });
        self.app.add_action(&refresh_action);
        self.app
            .set_accels_for_action("app.refresh", &["<Primary>r"]);

        // NOTE: "preferences" is registered by AppModel::init, because the
        // preferences component is owned by the app model's lifetime.
        // Registering it here would bind the action to a function that
        // cannot keep the component's runtime alive.

        let shortcuts_action = gtk::gio::SimpleAction::new("shortcuts", None);
        let app_clone = self.app.clone();
        shortcuts_action.connect_activate(move |_, _| {
            Self::show_shortcuts_window(app_clone.clone());
        });
        self.app.add_action(&shortcuts_action);
        self.app
            .set_accels_for_action("app.shortcuts", &["<Primary>question"]);

        let about_action = gtk::gio::SimpleAction::new("about", None);
        let app_clone = self.app.clone();
        about_action.connect_activate(move |_, _| {
            Self::show_about_window(app_clone.clone());
        });
        self.app.add_action(&about_action);

        let quit_action = gtk::gio::SimpleAction::new("quit", None);
        let app_clone = self.app.clone();
        quit_action.connect_activate(move |_, _| {
            app_clone.quit();
        });
        self.app.add_action(&quit_action);
        self.app.set_accels_for_action("app.quit", &["<Primary>q"]);
    }

    fn show_about_window(app: gtk::Application) {
        if let Some(active_window) = app.active_window() {
            let about = adw::AboutDialog::builder()
                .application_name(config::APPLICATION_NAME)
                .application_icon(config::APPLICATION_ID)
                .comments(gettext(
                    "Message your iPhone from your Linux desktop over Bluetooth.",
                ))
                .developer_name("Frost Edson")
                .website("https://github.com/edcorp844/iconnect")
                .issue_url("https://github.com/edcorp844/iconnect/issues")
                .copyright("© 2026 Frost")
                .license_type(gtk::License::Gpl30)
                .developers(vec!["Frost"])
                .designers(vec!["Frost"])
                .build();

            about.present(Some(&active_window));
        }
    }

    pub(crate) fn show_shortcuts_window(app: gtk::Application) {
        if let Some(active_window) = app.active_window() {
            let shortcuts_window = adw::ShortcutsDialog::builder()
                .title("Keyboard Shortcuts")
                .width_request(600)
                .height_request(500)
                .build();

            let window_section = adw::ShortcutsSection::new(Some(&gettext("Window")));
            let refresh = adw::ShortcutsItem::new(&gettext("Refresh"), "<Primary>r");
            refresh.set_subtitle(&gettext("Refresh all Application Content"));
            window_section.add(refresh);

            let quit = adw::ShortcutsItem::new(&gettext("Quit"), "<Primary>q");
            quit.set_subtitle(&gettext("Close the application"));
            window_section.add(quit);

            let application_section = adw::ShortcutsSection::new(Some(&gettext("Application")));
            let prefs = adw::ShortcutsItem::new(&gettext("Preferences"), "<Primary>comma");
            prefs.set_subtitle(&gettext("Configure application preferences"));
            application_section.add(prefs);

            let shorts = adw::ShortcutsItem::new(&gettext("Shortcuts"), "<Primary>question");
            shorts.set_subtitle(&gettext("Shows shortcuts window"));
            application_section.add(shorts);

            let nav_section = adw::ShortcutsSection::new(Some(&gettext("Navigation")));

            shortcuts_window.add(window_section);
            shortcuts_window.add(application_section);
            shortcuts_window.add(nav_section);

            shortcuts_window.present(Some(&active_window));
        }
    }
}