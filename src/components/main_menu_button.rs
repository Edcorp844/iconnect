use adw::prelude::*;
use gettextrs::gettext;
use relm4::{prelude::*, Component};

use crate::config;

pub struct MainMenuButton;

#[relm4::component(pub)]
impl Component for MainMenuButton {
    type Init = ();
    type Input = ();
    type Output = ();
    type CommandOutput = ();

    view! {
        gtk::MenuButton {
            set_icon_name: "open-menu-symbolic",
            set_tooltip_text: Some("Main Menu"),
            add_css_class: "flat",

            #[wrap(Some)]
            set_popover = &gtk::PopoverMenu::from_model(Some(&{
                let menu = gtk::gio::Menu::new();

                let window_section = gtk::gio::Menu::new();

                let refresh_item = gtk::gio::MenuItem::new(Some("Refresh"), Some("app.refresh"));
                refresh_item.set_attribute_value("accel", Some(&"<Primary>R".to_variant()));

                window_section.append_item(&refresh_item);


                menu.append_section(None, &window_section);

                let section = gtk::gio::Menu::new();

                let prefs_item = gtk::gio::MenuItem::new(Some("Preferences"), Some("app.preferences"));
                prefs_item.set_attribute_value("accel", Some(&"<Primary>comma".to_variant()));
                section.append_item(&prefs_item);

                let shortcuts_item = gtk::gio::MenuItem::new(Some("Keyboard Shortcuts"), Some("app.shortcuts"));
                shortcuts_item.set_attribute_value("accel", Some(&"<Primary>question".to_variant()));
                section.append_item(&shortcuts_item);

                let label = format!("{} {}", gettext("About"), config::APPLICATION_NAME);
                section.append(Some(&label), Some("app.about"));

                menu.append_section(None, &section);

                let quit_window_section = gtk::gio::Menu::new();
                let quit_window_item = gtk::gio::MenuItem::new(Some("Quit"), Some("app.quit"));
                quit_window_item.set_attribute_value("accel", Some(&"<Primary>Q".to_variant()));
                quit_window_section.append_item(&quit_window_item);
                menu.append_section(None, &quit_window_section);

                menu
            })) {}
        },
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        _sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = MainMenuButton {};

        let widgets = view_output!();

        ComponentParts { model, widgets }
    }
}
