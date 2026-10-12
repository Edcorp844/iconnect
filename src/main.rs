/* main.rs
 *
 * Copyright 2026 Frost
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <https://www.gnu.org/licenses/>.
 *
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

mod app_menu;
mod application;
mod components;
mod config;
mod workers;

use crate::{app_menu::AppMenu, application::AppModel};
use gettextrs::{bindtextdomain, setlocale, textdomain, LocaleCategory};
use relm4::RelmApp;

fn main() {
    setlocale(LocaleCategory::LcAll, "");
    bindtextdomain(config::GETTEXT_PACKAGE, config::LOCALEDIR)
        .expect("Unable to bind the text domain");
    textdomain(config::GETTEXT_PACKAGE).expect("Unable to switch to the text domain");

    let app_menu = AppMenu::new();
    app_menu.register();
    let app = RelmApp::new(config::APPLICATION_ID);
    let provider = gtk::CssProvider::new();

    let css_data = "
        @keyframes shimmer-flow {
            from { background-position: 0% 0%; }
            to { background-position: 200% 10%; }
        }

        @keyframes eq-bounce {
            0%   { transform: scaleY(0.3); }
            50%  { transform: scaleY(1.0); }
            100% { transform: scaleY(0.3); }
        }

        .eq-bar {
            background-color: white;
            min-width: 4px;
            min-height: 24px;
            border-radius: 2px;
            transform-origin: bottom;
            animation: eq-bounce 0.9s ease-in-out infinite;
        }

        .eq-bar-1 { animation-delay: 0s; }
        .eq-bar-2 { animation-delay: 0.15s; }
        .eq-bar-3 { animation-delay: 0.3s; }
        .eq-bar-4 { animation-delay: 0.45s; }
    ";

    provider.load_from_string(css_data);
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
    app.run::<AppModel>(());
}
