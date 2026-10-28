//! Custom-drawn widgets (GTK4 snapshot / GSK) and widgets composed from them.

pub mod gauge_arc;
pub mod interface_card;
pub mod rate_gauge;
pub mod status_pill;
pub mod traffic_chart;

/// Renders widgets to PNG to check their appearance (needs a graphical environment; briefly opens a window):
/// `ANEMOBIT_PREVIEW_DIR=/tmp/preview cargo test -p anemobit render_previews -- --ignored`
#[cfg(test)]
mod preview {
    use std::time::{Duration, Instant, SystemTime};

    use adw::prelude::*;
    use anemobit_core::formatting::RateUnitStyle;
    use anemobit_core::traffic::TrafficSample;
    use gtk::{gio, glib};

    use anemobit_core::SnmpError;

    use super::interface_card::InterfaceCard;
    use super::status_pill::StatusPill;
    use crate::config;
    use crate::theme::Palette;
    use crate::traffic_model::{ConnectionState, InterfaceTraffic};

    fn pump(duration: Duration) {
        let context = glib::MainContext::default();
        let until = Instant::now() + duration;
        while Instant::now() < until {
            context.iteration(false);
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn samples() -> Vec<TrafficSample> {
        let now = SystemTime::now();
        (0..300)
            .map(|i| {
                let t = i as f64;
                TrafficSample {
                    timestamp: now - Duration::from_secs(300 - i),
                    in_bps: 40e6
                        + 30e6 * (t / 25.0).sin()
                        + if i > 200 && i < 230 { 25e6 } else { 0.0 },
                    out_bps: 6e6 + 4e6 * (t / 40.0).cos(),
                    device_uptime: None,
                }
            })
            .collect()
    }

    fn render(widget: &impl IsA<gtk::Widget>, width: i32, path: &str) {
        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        content.add_css_class("anemobit-background");
        let widget = widget.as_ref();
        widget.set_margin_top(20);
        widget.set_margin_bottom(20);
        widget.set_margin_start(20);
        widget.set_margin_end(20);
        content.append(widget);
        let window = gtk::Window::builder()
            .child(&content)
            .default_width(width)
            .build();
        // Add (not replace) the class: replacing removes the background, and the text color comes out wrong.
        window.add_css_class("anemobit-main-window");
        window.present();
        pump(Duration::from_millis(1200));
        let paintable = gtk::WidgetPaintable::new(Some(&content));
        let snapshot = gtk::Snapshot::new();
        paintable.snapshot(&snapshot, content.width() as f64, content.height() as f64);
        let node = snapshot.to_node().unwrap();
        let texture = window.renderer().unwrap().render_texture(&node, None);
        texture.save_to_png(path).unwrap();
        window.destroy();
    }

    fn find_scrolled_window(widget: &gtk::Widget) -> Option<gtk::ScrolledWindow> {
        if let Some(scrolled) = widget.downcast_ref::<gtk::ScrolledWindow>() {
            return Some(scrolled.clone());
        }
        let mut child = widget.first_child();
        while let Some(current) = child {
            if let Some(found) = find_scrolled_window(&current) {
                return Some(found);
            }
            child = current.next_sibling();
        }
        None
    }

    fn interface(name: &str) -> InterfaceTraffic {
        let mut interface = InterfaceTraffic::new(name);
        for sample in samples() {
            interface.append(sample);
        }
        interface
    }

    #[test]
    #[ignore = "renders widgets to PNG files; needs a display"]
    fn render_previews() {
        let directory = std::env::var("ANEMOBIT_PREVIEW_DIR").expect("set ANEMOBIT_PREVIEW_DIR");
        // Same translation setup as main (the development build reads build/po); choose a language with LANGUAGE=ja etc.
        gettextrs::bindtextdomain(config::GETTEXT_PACKAGE, config::LOCALEDIR).unwrap();
        gettextrs::bind_textdomain_codeset(config::GETTEXT_PACKAGE, "UTF-8").unwrap();
        gettextrs::textdomain(config::GETTEXT_PACKAGE).unwrap();
        gtk::init().unwrap();
        adw::init().unwrap();
        gio::resources_register(
            &gio::Resource::from_data(&glib::Bytes::from_static(config::RESOURCES)).unwrap(),
        );
        let display = gtk::gdk::Display::default().unwrap();
        gtk::IconTheme::for_display(&display)
            .add_resource_path("/io/github/noctqira/Anemobit/icons");
        // Same as application: the development build looks for the app icon in data/icons in the source.
        gtk::IconTheme::for_display(&display).add_search_path(config::ICON_SEARCH_PATH);
        // A manually added stylesheet must set what @media (prefers-color-scheme) is based on; in the app, libadwaita handles it.
        let style = gtk::CssProvider::new();
        style.load_from_resource("/io/github/noctqira/Anemobit/style.css");
        gtk::style_context_add_provider_for_display(
            &display,
            &style,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
        let variables = gtk::CssProvider::new();
        gtk::style_context_add_provider_for_display(
            &display,
            &variables,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );

        // The compact window needs an application (which can only be created once).
        let app = adw::Application::new(None::<&str>, gio::ApplicationFlags::NON_UNIQUE);
        app.register(gio::Cancellable::NONE).unwrap();

        for (name, dark) in [("light", false), ("dark", true)] {
            adw::StyleManager::default().set_color_scheme(if dark {
                adw::ColorScheme::ForceDark
            } else {
                adw::ColorScheme::ForceLight
            });
            style.set_prefers_color_scheme(if dark {
                gtk::InterfaceColorScheme::Dark
            } else {
                gtk::InterfaceColorScheme::Light
            });
            let palette = Palette::new(dark, None, None);
            // Main window's translucent background (70% by default): transparent parts of the preview can be layered over any background to check legibility.
            variables.load_from_string(
                &(palette.css() + &crate::theme::translucency_css(dark, 0.7, 0.7)),
            );

            for (layout, compact, width) in [("wide", false, 800), ("compact", true, 400)] {
                let card = InterfaceCard::new("pppoe-out1");
                card.set_compact(compact);
                card.set_palette(&palette);
                card.update(
                    &interface("pppoe-out1"),
                    RateUnitStyle::Bits,
                    true,
                    SystemTime::now(),
                );
                render(
                    &card,
                    width,
                    &format!("{directory}/card-{layout}-{name}.png"),
                );
            }

            let pills = gtk::Box::new(gtk::Orientation::Horizontal, 12);
            for state in [
                ConnectionState::Connected,
                ConnectionState::Connecting,
                ConnectionState::Failed {
                    error: SnmpError::Timeout,
                    retry_in: Duration::from_secs(4),
                },
                ConnectionState::NotConfigured,
            ] {
                let pill = StatusPill::default();
                pill.set_state(&state);
                pills.append(&pill);
            }
            render(&pills, 500, &format!("{directory}/pills-{name}.png"));

            let dialog = crate::settings_dialog::SettingsDialog::new(
                &crate::traffic_model::TrafficModel::default(),
            );
            dialog.fill_preview();
            dialog.present(None::<&gtk::Widget>);
            pump(Duration::from_millis(1200));
            // The dialog's base color is drawn by the outer layer, so draw the whole dialog window. One shot at the top and one scrolled to the bottom.
            let root = dialog.root().unwrap();
            let save = |suffix: &str| {
                let paintable = gtk::WidgetPaintable::new(Some(&root));
                let snapshot = gtk::Snapshot::new();
                paintable.snapshot(&snapshot, f64::from(root.width()), f64::from(root.height()));
                let texture = root
                    .renderer()
                    .unwrap()
                    .render_texture(snapshot.to_node().unwrap(), None);
                texture
                    .save_to_png(format!("{directory}/settings-{suffix}-{name}.png"))
                    .unwrap();
            };
            save("top");
            if let Some(scrolled) = find_scrolled_window(dialog.upcast_ref()) {
                let adjustment = scrolled.vadjustment();
                adjustment.set_value(adjustment.upper());
                pump(Duration::from_millis(400));
            }
            save("bottom");
            dialog.force_close();

            // Compact window (as it looks when not set up) and the About window.
            let model = crate::traffic_model::TrafficModel::default();
            let compact = crate::compact_window::CompactWindow::new(&app, &model);
            compact.present();
            pump(Duration::from_millis(600));
            let paintable = gtk::WidgetPaintable::new(Some(&compact));
            let snapshot = gtk::Snapshot::new();
            paintable.snapshot(
                &snapshot,
                f64::from(compact.width()),
                f64::from(compact.height()),
            );
            let texture = compact
                .renderer()
                .unwrap()
                .render_texture(snapshot.to_node().unwrap(), None);
            texture
                .save_to_png(format!("{directory}/compact-{name}.png"))
                .unwrap();
            compact.destroy();

            let host = adw::Window::builder()
                .default_width(700)
                .default_height(700)
                .build();
            host.present();
            crate::about::show(Some(&host));
            pump(Duration::from_millis(1200));
            let paintable = gtk::WidgetPaintable::new(Some(&host));
            let snapshot = gtk::Snapshot::new();
            paintable.snapshot(&snapshot, f64::from(host.width()), f64::from(host.height()));
            let texture = host
                .renderer()
                .unwrap()
                .render_texture(snapshot.to_node().unwrap(), None);
            texture
                .save_to_png(format!("{directory}/about-{name}.png"))
                .unwrap();
            host.destroy();
        }
    }
}
