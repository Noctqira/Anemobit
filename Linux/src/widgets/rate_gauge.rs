//! Rate gauge: [`GaugeArc`] plus direction, value, unit and maximum text (same as RateGauge on macOS).

use std::cell::{Cell, OnceCell};

use adw::prelude::*;
use anemobit_core::formatting::{RateUnitStyle, format_rate, rate_parts};
use gettextrs::gettext;
use gtk::subclass::prelude::*;
use gtk::{CompositeTemplate, glib};

use super::gauge_arc::{GaugeArc, fraction};

use crate::theme::Palette;

/// Download or upload.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, glib::Enum)]
#[enum_type(name = "AnemobitTrafficDirection")]
pub enum TrafficDirection {
    #[default]
    Download,
    Upload,
}

impl TrafficDirection {
    /// Display name (localized).
    pub fn title(self) -> String {
        match self {
            Self::Download => gettext("Download"),
            Self::Upload => gettext("Upload"),
        }
    }

    pub fn icon_name(self) -> &'static str {
        match self {
            Self::Download => "anemobit-arrow-down-symbolic",
            Self::Upload => "anemobit-arrow-up-symbolic",
        }
    }

    /// CSS class for the text color (colors come from the variables in [`Palette::css`]).
    pub fn css_class(self) -> &'static str {
        match self {
            Self::Download => "anemobit-download",
            Self::Upload => "anemobit-upload",
        }
    }
}

mod imp {
    use super::*;

    #[derive(Debug, CompositeTemplate, glib::Properties)]
    #[template(resource = "/io/github/noctqira/Anemobit/ui/rate-gauge.ui")]
    #[properties(wrapper_type = super::RateGauge)]
    pub struct RateGauge {
        #[template_child]
        pub(super) arc: TemplateChild<GaugeArc>,
        #[template_child]
        pub(super) title_box: TemplateChild<gtk::Box>,
        #[template_child]
        pub(super) arrow: TemplateChild<gtk::Image>,
        #[template_child]
        pub(super) title_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) value_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) unit_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) max_label: TemplateChild<gtk::Label>,

        #[property(get, set = Self::set_direction, builder(TrafficDirection::Download))]
        direction: Cell<TrafficDirection>,
        /// Thinner ring and smaller text (for narrow layouts).
        #[property(get, set = Self::set_compact)]
        compact: Cell<bool>,

        pub(super) rate: Cell<Option<f64>>,
        pub(super) scale_max: Cell<f64>,
        pub(super) unit_style: Cell<RateUnitStyle>,
        pub(super) animation: OnceCell<adw::TimedAnimation>,
    }

    impl Default for RateGauge {
        fn default() -> Self {
            RateGauge {
                arc: TemplateChild::default(),
                title_box: TemplateChild::default(),
                arrow: TemplateChild::default(),
                title_label: TemplateChild::default(),
                value_label: TemplateChild::default(),
                unit_label: TemplateChild::default(),
                max_label: TemplateChild::default(),
                direction: Cell::new(TrafficDirection::Download),
                compact: Cell::new(false),
                rate: Cell::new(None),
                scale_max: Cell::new(anemobit_core::formatting::MINIMUM_RATE_SCALE),
                unit_style: Cell::new(RateUnitStyle::Bits),
                animation: OnceCell::new(),
            }
        }
    }

    impl RateGauge {
        fn set_direction(&self, direction: TrafficDirection) {
            let previous = self.direction.replace(direction);
            self.title_box.remove_css_class(previous.css_class());
            self.title_box.add_css_class(direction.css_class());
            self.arrow.set_icon_name(Some(direction.icon_name()));
            self.title_label.set_label(&direction.title());
            self.obj().refresh();
        }

        fn set_compact(&self, compact: bool) {
            self.compact.set(compact);
            self.arc.set_line_width(if compact { 10.0 } else { 14.0 });
            self.arc.set_natural_size(if compact { 170 } else { 200 });
            if compact {
                self.obj().add_css_class("compact");
            } else {
                self.obj().remove_css_class("compact");
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for RateGauge {
        const NAME: &'static str = "AnemobitRateGauge";
        type Type = super::RateGauge;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            GaugeArc::ensure_type();
            klass.bind_template();
            klass.set_layout_manager_type::<gtk::BinLayout>();
            klass.set_css_name("anemobit-rate-gauge");
            klass.set_accessible_role(gtk::AccessibleRole::Meter);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for RateGauge {
        fn constructed(&self) {
            self.parent_constructed();
            let target = adw::PropertyAnimationTarget::new(&*self.arc, "fraction");
            let animation = adw::TimedAnimation::new(&*self.obj(), 0.0, 0.0, 600, target);
            animation.set_easing(adw::Easing::EaseOutCubic);
            self.animation
                .set(animation)
                .expect("constructed runs once");
            self.set_direction(self.direction.get());
        }

        fn dispose(&self) {
            self.dispose_template();
            // The template's outermost GtkOverlay has no id, so dispose_template doesn't handle it; remove it manually.
            while let Some(child) = self.obj().first_child() {
                child.unparent();
            }
        }
    }

    impl WidgetImpl for RateGauge {}
}

glib::wrapper! {
    pub struct RateGauge(ObjectSubclass<imp::RateGauge>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl RateGauge {
    pub fn new(direction: TrafficDirection) -> Self {
        glib::Object::builder()
            .property("direction", direction)
            .build()
    }

    /// Current rate (bps); `None` means no data yet.
    pub fn set_rate(&self, bps: Option<f64>) {
        self.imp().rate.set(bps);
        self.refresh();
    }

    /// Gauge maximum (bps), usually the result of [`anemobit_core::formatting::nice_ceiling`].
    pub fn set_scale_max(&self, bps: f64) {
        self.imp().scale_max.set(bps);
        self.refresh();
    }

    pub fn set_unit_style(&self, style: RateUnitStyle) {
        self.imp().unit_style.set(style);
        self.refresh();
    }

    pub fn set_palette(&self, palette: &Palette) {
        let color = match self.direction() {
            TrafficDirection::Download => palette.download,
            TrafficDirection::Upload => palette.upload,
        };
        self.imp()
            .arc
            .set_colors(color, palette.track, if palette.dark { 0.7 } else { 0.35 });
    }

    /// Updates the text, accessibility info and ring (animated) from the current rate, maximum and unit.
    fn refresh(&self) {
        let imp = self.imp();
        let style = imp.unit_style.get();
        let scale_max = imp.scale_max.get();
        let rate = imp.rate.get();
        match rate {
            Some(bps) => {
                let parts = rate_parts(bps, style);
                imp.value_label.set_label(&parts.value);
                imp.unit_label.set_label(parts.unit);
            }
            None => {
                imp.value_label.set_label("--");
                imp.unit_label.set_label(" ");
            }
        }
        // Translators: {} is a rate such as "100.0 Mbps".
        imp.max_label.set_label(&gettext("Max {}").replacen(
            "{}",
            &format_rate(scale_max, style),
            1,
        ));

        let value_text = match rate {
            Some(bps) => format_rate(bps, style),
            None => gettext("No data"),
        };
        self.update_property(&[
            gtk::accessible::Property::Label(&self.direction().title()),
            gtk::accessible::Property::ValueMin(0.0),
            gtk::accessible::Property::ValueMax(scale_max),
            gtk::accessible::Property::ValueNow(rate.unwrap_or(0.0)),
            gtk::accessible::Property::ValueText(&value_text),
        ]);

        if let Some(animation) = imp.animation.get() {
            animation.set_value_from(imp.arc.fraction());
            animation.set_value_to(fraction(rate, scale_max));
            animation.play();
        }
    }
}
