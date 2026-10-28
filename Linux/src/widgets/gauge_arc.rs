//! Ring of the 270° gauge (same as macOS: angular gradient and glow); [`super::rate_gauge::RateGauge`] overlays the text.

use std::cell::{Cell, RefCell};

use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk, glib, graphene, gsk};

/// Start of the ring (screen angle, clockwise from +x; 135° is bottom left) and the sweep angle.
const START_DEGREES: f32 = 135.0;
const SWEEP_DEGREES: f32 = 270.0;

/// Rate as a fraction of the gauge maximum, clamped to 0–1; 0 when there's no data or the maximum is unreasonable.
pub fn fraction(bps: Option<f64>, scale_max: f64) -> f64 {
    match bps {
        Some(bps) if bps.is_finite() && scale_max.is_finite() && scale_max > 0.0 => {
            (bps / scale_max).clamp(0.0, 1.0)
        }
        _ => 0.0,
    }
}

/// One segment of an arc: a conic (rational quadratic Bézier), a true circle when `weight` = cos(segment angle / 2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ArcSegment {
    pub control: (f32, f32),
    pub end: (f32, f32),
    pub weight: f32,
}

/// Center `(cx, cy)`, radius `radius`, sweeping `sweep` degrees clockwise from `start` (screen angles);
/// returns the start point and conic segments of at most 90° each.
pub fn arc_segments(
    cx: f32,
    cy: f32,
    radius: f32,
    start: f32,
    sweep: f32,
) -> ((f32, f32), Vec<ArcSegment>) {
    let point = |degrees: f32, distance: f32| {
        let radians = degrees.to_radians();
        (cx + distance * radians.cos(), cy + distance * radians.sin())
    };
    let begin = point(start, radius);
    if sweep <= 0.0 {
        return (begin, Vec::new());
    }
    let count = (sweep / 90.0).ceil();
    let step = sweep / count;
    let half = (step / 2.0).to_radians();
    let segments = (0..count as u32)
        .map(|i| {
            let from = start + step * i as f32;
            ArcSegment {
                // The control point is where the end tangents meet: r / cos(half angle) from the center along the bisector.
                control: point(from + step / 2.0, radius / half.cos()),
                end: point(from + step, radius),
                weight: half.cos(),
            }
        })
        .collect();
    (begin, segments)
}

fn arc_path(cx: f32, cy: f32, radius: f32, start: f32, sweep: f32) -> gsk::Path {
    let builder = gsk::PathBuilder::new();
    let (begin, segments) = arc_segments(cx, cy, radius, start, sweep);
    builder.move_to(begin.0, begin.1);
    for segment in segments {
        builder.conic_to(
            segment.control.0,
            segment.control.1,
            segment.end.0,
            segment.end.1,
            segment.weight,
        );
    }
    builder.to_path()
}

/// Draws the gauge ring. `size` is the side of the square; the ring touches the edges (the glow may extend outside).
pub fn draw(
    snapshot: &gtk::Snapshot,
    size: f32,
    line_width: f32,
    fraction: f32,
    color: &gdk::RGBA,
    track: &gdk::RGBA,
    glow: f32,
) {
    let center = size / 2.0;
    let radius = (size - line_width) / 2.0;
    let stroke = gsk::Stroke::new(line_width);
    stroke.set_line_cap(gsk::LineCap::Round);

    snapshot.append_stroke(
        &arc_path(center, center, radius, START_DEGREES, SWEEP_DEGREES),
        &stroke,
        track,
    );

    let fraction = fraction.clamp(0.0, 1.0);
    if fraction <= 0.0 {
        return;
    }
    let mut faded = *color;
    faded.set_alpha(color.alpha() * 0.45);
    let mut shadow = *color;
    shadow.set_alpha(glow);
    let path = arc_path(
        center,
        center,
        radius,
        START_DEGREES,
        SWEEP_DEGREES * fraction,
    );
    let bounds = graphene::Rect::new(
        -line_width,
        -line_width,
        size + 2.0 * line_width,
        size + 2.0 * line_width,
    );

    snapshot.push_shadow(&[gsk::Shadow::new(shadow, 0.0, 0.0, 8.0)]);
    snapshot.push_stroke(&path, &stroke);
    // Angular gradient: 0° at the top, clockwise; the ring's start (screen 135°) corresponds to 225°. The whole ring always goes from light to strong (same as macOS).
    snapshot.append_conic_gradient(
        &bounds,
        &graphene::Point::new(center, center),
        START_DEGREES + 90.0,
        &[
            gsk::ColorStop::new(0.0, faded),
            gsk::ColorStop::new(SWEEP_DEGREES / 360.0, *color),
            gsk::ColorStop::new(1.0, *color),
        ],
    );
    snapshot.pop();
    snapshot.pop();
}

mod imp {
    use super::*;

    #[derive(Debug, glib::Properties)]
    #[properties(wrapper_type = super::GaugeArc)]
    pub struct GaugeArc {
        /// 0–1, animated by [`crate::widgets::rate_gauge::RateGauge`].
        #[property(get, set = Self::set_fraction, minimum = 0.0, maximum = 1.0)]
        fraction: Cell<f64>,
        #[property(get, set = Self::set_line_width)]
        line_width: Cell<f64>,
        /// Preferred side length (same as macOS: 200 normally, 170 compact); the actual size is the short side of the allocation.
        #[property(get, set = Self::set_natural_size)]
        natural_size: Cell<i32>,
        pub(super) color: RefCell<gdk::RGBA>,
        pub(super) track: RefCell<gdk::RGBA>,
        /// Glow opacity (stronger in dark mode).
        pub(super) glow: Cell<f32>,
    }

    impl Default for GaugeArc {
        fn default() -> Self {
            GaugeArc {
                fraction: Cell::new(0.0),
                line_width: Cell::new(14.0),
                natural_size: Cell::new(200),
                color: RefCell::new(gdk::RGBA::BLACK),
                track: RefCell::new(gdk::RGBA::new(0.0, 0.0, 0.0, 0.08)),
                glow: Cell::new(0.35),
            }
        }
    }

    impl GaugeArc {
        fn set_fraction(&self, fraction: f64) {
            self.fraction.set(fraction);
            self.obj().queue_draw();
        }

        fn set_line_width(&self, width: f64) {
            self.line_width.set(width);
            self.obj().queue_draw();
        }

        fn set_natural_size(&self, size: i32) {
            self.natural_size.set(size);
            self.obj().queue_resize();
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for GaugeArc {
        const NAME: &'static str = "AnemobitGaugeArc";
        type Type = super::GaugeArc;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_accessible_role(gtk::AccessibleRole::Presentation);
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for GaugeArc {}

    impl WidgetImpl for GaugeArc {
        /// Both directions have a minimum of 96 and prefer `natural_size`; drawing uses the short side as a centered square.
        fn measure(&self, _orientation: gtk::Orientation, _for_size: i32) -> (i32, i32, i32, i32) {
            let minimum = 96;
            (minimum, self.natural_size.get().max(minimum), -1, -1)
        }

        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let widget = self.obj();
            let size = widget.width().min(widget.height()) as f32;
            if size <= 0.0 {
                return;
            }
            // Center within the allocated area.
            snapshot.translate(&graphene::Point::new(
                (widget.width() as f32 - size) / 2.0,
                (widget.height() as f32 - size) / 2.0,
            ));
            draw(
                snapshot,
                size,
                self.line_width.get() as f32,
                self.fraction.get() as f32,
                &self.color.borrow(),
                &self.track.borrow(),
                self.glow.get(),
            );
        }
    }
}

glib::wrapper! {
    pub struct GaugeArc(ObjectSubclass<imp::GaugeArc>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl GaugeArc {
    /// Sets the colors: value color, track color, glow opacity.
    pub fn set_colors(&self, color: gdk::RGBA, track: gdk::RGBA, glow: f32) {
        let imp = self.imp();
        imp.color.replace(color);
        imp.track.replace(track);
        imp.glow.set(glow);
        self.queue_draw();
    }
}

impl Default for GaugeArc {
    fn default() -> Self {
        glib::Object::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-3
    }

    #[test]
    fn fraction_is_clamped() {
        assert_eq!(fraction(None, 1_000.0), 0.0);
        assert_eq!(fraction(Some(500.0), 1_000.0), 0.5);
        assert_eq!(fraction(Some(2_000.0), 1_000.0), 1.0);
        assert_eq!(fraction(Some(-1.0), 1_000.0), 0.0);
        assert_eq!(fraction(Some(f64::NAN), 1_000.0), 0.0);
        assert_eq!(fraction(Some(500.0), 0.0), 0.0);
    }

    #[test]
    fn full_sweep_starts_bottom_left_and_ends_bottom_right() {
        let (start, segments) = arc_segments(0.0, 0.0, 10.0, START_DEGREES, SWEEP_DEGREES);
        let d = 10.0 * std::f32::consts::FRAC_1_SQRT_2;
        assert!(close(start.0, -d) && close(start.1, d), "{start:?}");
        assert_eq!(segments.len(), 3);
        let end = segments.last().unwrap().end;
        assert!(close(end.0, d) && close(end.1, d), "{end:?}");
        // 90° per segment: weight = cos 45°
        assert!(
            segments
                .iter()
                .all(|s| close(s.weight, std::f32::consts::FRAC_1_SQRT_2))
        );
    }

    #[test]
    fn segments_end_on_the_circle_and_split_at_most_90_degrees() {
        let (_, segments) = arc_segments(5.0, 5.0, 20.0, START_DEGREES, 100.0);
        assert_eq!(segments.len(), 2);
        for segment in &segments {
            let r = ((segment.end.0 - 5.0).powi(2) + (segment.end.1 - 5.0).powi(2)).sqrt();
            assert!(close(r, 20.0));
            assert!(segment.weight >= std::f32::consts::FRAC_1_SQRT_2 - 1e-6);
        }
        // y is smallest when passing the top (screen 270°).
        let (_, half) = arc_segments(0.0, 0.0, 10.0, 180.0, 90.0);
        assert!(close(half[0].end.0, 0.0) && close(half[0].end.1, -10.0));
    }

    #[test]
    fn empty_sweep_has_no_segments() {
        let (_, segments) = arc_segments(0.0, 0.0, 10.0, START_DEGREES, 0.0);
        assert!(segments.is_empty());
    }
}
