use adw::prelude::*;
use relm4::{prelude::*, Component};

pub struct MessageTile {
    name: String,
    last_message: String,
}

#[relm4::component(pub)]
impl Component for MessageTile {
    type Init = (String, String);
    type Input = ();
    type Output = ();
    type CommandOutput = ();

    view! {
        adw::SidebarItem {
            set_title: Some(&model.name),
            set_subtitle: Some(&model.last_message),
        }
    }

    fn init(
        init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let (name, last_message) = init;
        let model = MessageTile { name, last_message };
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }
}
