//! Card for one interface: name, download / upload gauges, statistics (current / average / peak), and an optional 5-minute chart.
//! Same as InterfaceCard on macOS; in narrow layouts the gauges shrink and statistics go below them.

use std::cell::Cell;
use std::time::SystemTime;

use anemobit_core::formatting::{RateUnitStyle, format_rate};
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{CompositeTemplate, glib};

use super::rate_gauge::{RateGauge, TrafficDirection};
use super::traffic_chart::TrafficChart;
use crate::theme::Palette;
use crate::traffic_model::InterfaceTraffic;

mod imp {
    use super::*;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/io/github/noctqira/Anemobit/ui/interface-card.ui")]
    pub struct InterfaceCard {
        #[template_child]
        pub(super) name_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) body: TemplateChild<gtk::Box>,
        #[template_child]
        pub(super) download_gauge: TemplateChild<RateGauge>,
        #[template_child]
        pub(super) upload_gauge: TemplateChild<RateGauge>,
        #[template_child]
        pub(super) stats: TemplateChild<gtk::Grid>,
        #[template_child]
        pub(super) stats_download_title: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) stats_upload_title: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) current_in: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) current_out: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) average_in: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) average_out: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) peak_in: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) peak_out: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) chart_section: TemplateChild<gtk::Box>,
        #[template_child]
        pub(super) legend_download: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) legend_upload: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) chart: TemplateChild<TrafficChart>,
        pub(super) compact: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for InterfaceCard {
        const NAME: &'static str = "AnemobitInterfaceCard";
        type Type = super::InterfaceCard;
        type ParentType = gtk::Box;

        fn class_init(klass: &mut Self::Class) {
            RateGauge::ensure_type();
            TrafficChart::ensure_type();
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for InterfaceCard {
        fn constructed(&self) {
            self.parent_constructed();
            let download = TrafficDirection::Download.title();
            let upload = TrafficDirection::Upload.title();
            self.stats_download_title.set_label(&download);
            self.legend_download.set_label(&download);
            self.stats_upload_title.set_label(&upload);
            self.legend_upload.set_label(&upload);
        }
    }

    impl WidgetImpl for InterfaceCard {}
    impl BoxImpl for InterfaceCard {}
}

glib::wrapper! {
    pub struct InterfaceCard(ObjectSubclass<imp::InterfaceCard>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl InterfaceCard {
    pub fn new(name: &str) -> Self {
        let card: Self = glib::Object::new();
        card.imp().name_label.set_label(name);
        card
    }

    /// Narrow layout: smaller gauges, statistics below the gauges, a taller chart without a time axis.
    pub fn set_compact(&self, compact: bool) {
        let imp = self.imp();
        imp.compact.set(compact);
        imp.body.set_orientation(if compact {
            gtk::Orientation::Vertical
        } else {
            gtk::Orientation::Horizontal
        });
        imp.body.set_spacing(if compact { 12 } else { 24 });
        self.set_spacing(if compact { 12 } else { 16 });
        imp.download_gauge.set_compact(compact);
        imp.upload_gauge.set_compact(compact);
        imp.chart.set_compact(compact);
        imp.chart
            .set_height_request(if compact { 170 } else { 160 });
        imp.stats.set_halign(if compact {
            gtk::Align::Fill
        } else {
            gtk::Align::Center
        });
    }

    pub fn set_palette(&self, palette: &Palette) {
        let imp = self.imp();
        imp.download_gauge.set_palette(palette);
        imp.upload_gauge.set_palette(palette);
        imp.chart.set_palette(palette);
    }

    /// Updates from the interface's latest data; hides the chart when `shows_chart` is false.
    pub fn update(
        &self,
        interface: &InterfaceTraffic,
        style: RateUnitStyle,
        shows_chart: bool,
        now: SystemTime,
    ) {
        let imp = self.imp();
        let latest = interface.latest();
        for (gauge, direction, rate) in [
            (
                &imp.download_gauge,
                TrafficDirection::Download,
                latest.map(|s| s.in_bps),
            ),
            (
                &imp.upload_gauge,
                TrafficDirection::Upload,
                latest.map(|s| s.out_bps),
            ),
        ] {
            gauge.set_unit_style(style);
            gauge.set_scale_max(interface.gauge_scale(direction, style, now));
            gauge.set_rate(rate);
        }

        let statistics = interface.statistics(now);
        let value =
            |bps: Option<f64>| bps.map_or_else(|| "--".to_owned(), |bps| format_rate(bps, style));
        imp.current_in.set_label(&value(latest.map(|s| s.in_bps)));
        imp.current_out.set_label(&value(latest.map(|s| s.out_bps)));
        imp.average_in
            .set_label(&value(statistics.map(|s| s.average_in)));
        imp.average_out
            .set_label(&value(statistics.map(|s| s.average_out)));
        imp.peak_in.set_label(&value(statistics.map(|s| s.peak_in)));
        imp.peak_out
            .set_label(&value(statistics.map(|s| s.peak_out)));

        imp.chart_section.set_visible(shows_chart);
        if shows_chart {
            imp.chart.set_unit_style(style);
            imp.chart.set_samples(interface.history().cloned());
        }
    }
}
