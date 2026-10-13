use adw::prelude::*;
use chrono::{DateTime, Local, Utc};
use gtk::glib;
use relm4::{prelude::*, Component, ComponentParts, ComponentSender, RelmRemoveAllExt};

use crate::workers::message_thread::{Message, Thread};

pub struct MessageThreadWidget {
    thread: Option<Thread>,
}

#[derive(Debug, Clone)]
pub enum MessageThreadInput {
    SendMessage,
    LoadThread(Thread),
}

#[derive(Debug, Clone)]
pub enum MessageThreadOutput {
    SendMessage { key: String, body: String },
}

#[relm4::component(pub)]
impl Component for MessageThreadWidget {
    type Init = ();
    type Input = MessageThreadInput;
    type Output = MessageThreadOutput;
    type CommandOutput = ();

    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_hexpand: true,
            set_vexpand: true,

            gtk::ScrolledWindow {
                set_vexpand: true,
                set_hexpand: true,
                set_policy: (gtk::PolicyType::Never, gtk::PolicyType::Automatic),

                #[wrap(Some)]
                set_child = &gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_spacing: 8,
                    set_margin_all: 12,
                    set_valign: gtk::Align::End,

                    #[name = "messages_box"]
                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 6,
                    }
                }
            },

            // Compose bar
            gtk::Box {
                set_orientation: gtk::Orientation::Horizontal,
                set_spacing: 8,
                set_margin_all: 10,

                gtk::Button {
                    set_icon_name: "list-add-symbolic",
                    set_valign: gtk::Align::Center,
                },

                #[name="scrolled_window"]
                gtk::ScrolledWindow {
                    set_hexpand: true,
                    set_max_content_height: 120,
                    set_propagate_natural_height: true,
                    set_policy: (gtk::PolicyType::Never, gtk::PolicyType::Automatic),

                    #[wrap(Some)]
                    #[name="message_text_box"]
                    set_child = &gtk::TextView {
                        set_wrap_mode: gtk::WrapMode::WordChar,
                        set_top_margin: 6,
                        set_bottom_margin: 6,
                        add_css_class: "frame",
                        set_left_margin: 8,
                        set_right_margin: 8,
                        inline_css: "background-color: var(--card-bg-color); border-radius: 15px;"
                    }
                },
                gtk::Button {
                    set_icon_name: "send-symbolic",
                    set_valign: gtk::Align::Center,

                    connect_clicked[sender] => move |_| {
                        sender.input(MessageThreadInput::SendMessage);
                    }
                },
                gtk::Button {
                    set_icon_name: "face-smile-symbolic",
                    set_valign: gtk::Align::Center,
                },
            }
        }
    }

    fn init(
        _init: Self::Init,
        _root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = Self { thread: None };
        let widgets = view_output!();
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
            MessageThreadInput::SendMessage => {
                if let Some(thread) = &self.thread {
                    let key = &thread.key;
                    let buffer = widgets.message_text_box.buffer();
                    let text = buffer
                        .text(&buffer.start_iter(), &buffer.end_iter(), false)
                        .to_string();
                    if text.trim().is_empty() {
                        return;
                    }
                    let _ = sender.output(MessageThreadOutput::SendMessage {
                        key: key.clone(),
                        body: text,
                    });
                    widgets.message_text_box.buffer().set_text("");
                }
            }
            MessageThreadInput::LoadThread(thread) => {
                self.thread = Some(thread);
                widgets.messages_box.remove_all();

                if let Some(thread) = &self.thread {
                    let mut last_day: Option<String> = None;

                    for msg in &thread.messages {
                        let day = day_header(&msg.timestamp);

                        // Insert a day header whenever the day changes
                        if last_day.as_ref() != Some(&day) {
                            let header = build_day_header(&day);
                            widgets.messages_box.append(&header);
                            last_day = Some(day);
                        }

                        widgets.messages_box.append(&build_canvas_bubble(msg));
                    }
                }

                // Scroll to bottom after layout
                let adj = widgets.scrolled_window.vadjustment();
                glib::idle_add_local_once(move || {
                    glib::idle_add_local_once(move || {
                        let target = adj.upper() - adj.page_size();
                        adj.set_value(target);
                    });
                });
            }
        }
    }
}

// ─────────────────────────────────────────────
// Day header (Today / Yesterday / Thursday / Oct 12, 2025)
// ─────────────────────────────────────────────
fn day_header(iso_str: &str) -> String {
    let Ok(utc_dt) = DateTime::parse_from_rfc3339(iso_str) else {
        return iso_str.to_string();
    };

    let local_dt = utc_dt.with_timezone(&Local);
    let now = Local::now();
    let dt_date = local_dt.date_naive();
    let today = now.date_naive();

    if dt_date == today {
        "Today".to_string()
    } else if dt_date == today - chrono::Duration::try_days(1).unwrap() {
        "Yesterday".to_string()
    } else if today.signed_duration_since(dt_date).num_days() < 7 {
        local_dt.format("%A").to_string() // Thursday, Friday…
    } else {
        local_dt.format("%b %d, %Y").to_string() // Oct 12, 2025
    }
}

fn build_day_header(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.set_halign(gtk::Align::Center);
    label.set_margin_top(16);
    label.set_margin_bottom(8);
    label.set_opacity(0.55);
    label.add_css_class("caption");
    // Optional: make it a bit stronger
    // label.add_css_class("heading");
    label
}

// ─────────────────────────────────────────────
// Time only (9:19 PM)
// ─────────────────────────────────────────────
fn time_only(iso_str: &str) -> String {
    let Ok(utc_dt) = DateTime::parse_from_rfc3339(iso_str) else {
        return iso_str.to_string();
    };

    let local_dt = utc_dt.with_timezone(&Local);
    let time_str = local_dt.format("%I:%M %p").to_string();
    time_str.strip_prefix('0').unwrap_or(&time_str).to_string()
}

// ─────────────────────────────────────────────
// Bubble (now only shows the time)
// ─────────────────────────────────────────────
fn build_canvas_bubble(msg: &Message) -> gtk::Box {
    let is_outgoing = msg.outgoing;
    let body = msg.body.clone();
    let timestamp = time_only(&msg.timestamp); // ← only the time now

    let row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    row.set_hexpand(true);

    if is_outgoing {
        let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        spacer.set_hexpand(true);
        row.append(&spacer);
    }

    let container = gtk::Box::new(gtk::Orientation::Vertical, 3);
    container.set_halign(if is_outgoing {
        gtk::Align::End
    } else {
        gtk::Align::Start
    });

    // ---------- Bubble Overlay ----------
    let overlay = gtk::Overlay::new();
    overlay.set_hexpand(false);
    overlay.set_vexpand(false);

    // ---------- Label ----------
    let label = gtk::Label::new(None);
    label.set_wrap(true);
    label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    label.add_css_class("document");
    label.set_max_width_chars(32);
    label.set_xalign(0.0);
    label.set_yalign(0.5);
    label.set_halign(gtk::Align::Fill);
    label.set_valign(gtk::Align::Center);

    label.set_margin_start(if is_outgoing { 16 } else { 16 + 9 });
    label.set_margin_end(if is_outgoing { 16 + 9 } else { 16 });
    label.set_margin_top(10);
    label.set_margin_bottom(10);

    if is_outgoing {
        let escaped = glib::markup_escape_text(body.trim());
        label.set_use_markup(true);
        label.set_label(&format!(r#"<span foreground="white">{}</span>"#, escaped));
    } else {
        label.set_use_markup(false);
        label.set_text(body.trim());
    }

    // ---------- DrawingArea ----------
    let area = gtk::DrawingArea::new();
    area.set_can_target(false);

    let outgoing = is_outgoing;
    let tail = 9.0;
    let tail_f = tail as f64;

    area.set_draw_func(move |da, cr, width, height| {
        let w = width as f64;
        let h = height as f64;
        let r = 18.0;

        if outgoing {
            let color = theme_accent(da);
            cr.set_source_rgba(
                color.red() as f64,
                color.green() as f64,
                color.blue() as f64,
                1.0,
            );
        } else {
            cr.set_source_rgba(0.30, 0.30, 0.32, 0.2);
        }

        cr.new_path();
        if outgoing {
            // Body is inset on the right by the tail width
            cr.move_to(r, 0.0);
            cr.line_to(w - r - tail_f, 0.0);
            cr.arc(w - r - tail_f, r, r, -std::f64::consts::FRAC_PI_2, 0.0);
            cr.line_to(w - tail_f, h - r);
            //cr.arc(w - r - tail_f, h - r, r, 0.0, std::f64::consts::FRAC_PI_2);

            // Bottom edge to the base of the tail
            cr.line_to(w - tail_f, h);

            // Short right tail (stays inside the widget)
            cr.line_to(w, h - 15.0); // tip (uses the extra margin)
            cr.line_to(w - tail_f, h - 25.0);
            cr.line_to(w - tail_f, h - r);

            // Continue to left side
            cr.line_to(w - tail_f, h);
            cr.arc(
                r,
                h - r,
                r,
                std::f64::consts::FRAC_PI_2,
                std::f64::consts::PI,
            );
            cr.line_to(0.0, r);
            cr.arc(r, r, r, std::f64::consts::PI, 1.5 * std::f64::consts::PI);
        } else {
            cr.move_to(r + tail_f, 0.0);
            cr.line_to(w - r, 0.0);
            cr.arc(w - r, r, r, -std::f64::consts::FRAC_PI_2, 0.0);
            cr.line_to(w, h - r);
            cr.arc(w - r, h - r, r, 0.0, std::f64::consts::FRAC_PI_2);
            cr.line_to(r + tail_f, h);

            cr.line_to(0.0, h - 15.0);
            cr.line_to(tail_f, h - 25.0);
            cr.line_to(tail_f, h - r);

            cr.line_to(tail_f, r);
            cr.arc(
                r + tail_f,
                r,
                r,
                std::f64::consts::PI,
                1.5 * std::f64::consts::PI,
            );
        }

        cr.close_path();
        let _ = cr.fill();
    });

    overlay.set_child(Some(&area));
    overlay.add_overlay(&label);
    overlay.set_measure_overlay(&label, true);

    container.append(&overlay);

    // ---------- Time only (under the bubble) ----------
    let time = gtk::Label::new(Some(&timestamp));
    time.set_opacity(0.55);
    time.set_halign(if is_outgoing {
        gtk::Align::End
    } else {
        gtk::Align::Start
    });
    time.set_margin_start(8);
    time.set_margin_end(8);
    time.add_css_class("caption");
    time.set_hexpand(false);
    time.set_ellipsize(gtk::pango::EllipsizeMode::End);
    time.set_max_width_chars(12);

    container.append(&time);

    row.append(&container);
    row.set_margin_bottom(12); // a bit tighter now that we have day headers

    if !is_outgoing {
        let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        spacer.set_hexpand(true);
        row.append(&spacer);
    }

    row
}

fn theme_accent(widget: &gtk::DrawingArea) -> gtk::gdk::RGBA {
    let context = widget.style_context();
    context
        .lookup_color("accent_color")
        .unwrap_or_else(|| context.color())
}
