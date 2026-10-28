//! Area chart of download / upload over the last 5 minutes (same as TrafficAreaChart on macOS).

use std::cell::{Cell, RefCell};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anemobit_core::formatting::{RateUnitStyle, format_rate, rate_axis};
use anemobit_core::traffic::TrafficSample;
use gettextrs::gettext;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk, glib, graphene, gsk, pango};

use crate::theme::Palette;

/// Time span shown by the chart.
pub const HISTORY_WINDOW: Duration = Duration::from_secs(5 * 60);

/// One cubic Bézier segment: two control points and the end point.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CubicSegment {
    pub control1: (f32, f32),
    pub control2: (f32, f32),
    pub end: (f32, f32),
}

/// Monotone cubic interpolation (like d3's curveMonotoneX, Steffen's method): smooth curves that never overshoot neighboring data points.
/// The x values of `points` must increase; returns the segments starting from the first point.
pub fn monotone_curve(points: &[(f32, f32)]) -> Vec<CubicSegment> {
    if points.len() < 2 {
        return Vec::new();
    }
    let slope = |a: (f32, f32), b: (f32, f32)| {
        let h = b.0 - a.0;
        if h == 0.0 { 0.0 } else { (b.1 - a.1) / h }
    };
    // d3's sign: 0 counts as positive.
    let sign = |v: f32| if v < 0.0 { -1.0 } else { 1.0 };
    let slopes: Vec<f32> = points.windows(2).map(|w| slope(w[0], w[1])).collect();

    // Tangent at each point. Inner points: the minimum when both slopes have the same sign (including Steffen's weighted average limit), 0 when they differ.
    let mut tangents = vec![0.0f32; points.len()];
    for i in 1..points.len() - 1 {
        let (h0, h1) = (points[i].0 - points[i - 1].0, points[i + 1].0 - points[i].0);
        let (s0, s1) = (slopes[i - 1], slopes[i]);
        let p = (s0 * h1 + s1 * h0) / (h0 + h1);
        let t = (sign(s0) + sign(s1)) * s0.abs().min(s1.abs()).min(0.5 * p.abs());
        tangents[i] = if t.is_finite() { t } else { 0.0 };
    }
    let last = points.len() - 1;
    if points.len() == 2 {
        tangents = vec![slopes[0], slopes[0]];
    } else {
        // End points: derived from the adjacent tangent (d3's slope2).
        tangents[0] = (3.0 * slopes[0] - tangents[1]) / 2.0;
        tangents[last] = (3.0 * slopes[last - 1] - tangents[last - 1]) / 2.0;
    }

    points
        .windows(2)
        .enumerate()
        .map(|(i, w)| {
            let (p0, p1) = (w[0], w[1]);
            let dx = (p1.0 - p0.0) / 3.0;
            CubicSegment {
                control1: (p0.0 + dx, p0.1 + dx * tangents[i]),
                control2: (p1.0 - dx, p1.1 - dx * tangents[i + 1]),
                end: p1,
            }
        })
        .collect()
}

/// Whole-minute times between `[start, end]` (Unix seconds).
pub fn minute_ticks(start: f64, end: f64) -> Vec<f64> {
    let mut ticks = Vec::new();
    let mut minute = (start / 60.0).ceil() * 60.0;
    while minute <= end {
        ticks.push(minute);
        minute += 60.0;
    }
    ticks
}

/// Mapping between the plot area and the data range.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Plot {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    /// Time range (Unix seconds).
    pub start: f64,
    pub end: f64,
    /// Vertical axis maximum (bps).
    pub maximum: f64,
}

impl Plot {
    pub fn x_for(&self, time: f64) -> f32 {
        let span = self.end - self.start;
        if span <= 0.0 {
            return self.x;
        }
        self.x + ((time - self.start) / span) as f32 * self.width
    }

    pub fn y_for(&self, bps: f64) -> f32 {
        if self.maximum <= 0.0 {
            return self.y + self.height;
        }
        self.y + self.height - (bps.max(0.0) / self.maximum) as f32 * self.height
    }
}

/// Curve starting at `first`; when `baseline` is set, it continues down to the baseline and closes (for areas).
fn curve_path(first: (f32, f32), curve: &[CubicSegment], baseline: Option<f32>) -> gsk::Path {
    let builder = gsk::PathBuilder::new();
    builder.move_to(first.0, first.1);
    for segment in curve {
        builder.cubic_to(
            segment.control1.0,
            segment.control1.1,
            segment.control2.0,
            segment.control2.1,
            segment.end.0,
            segment.end.1,
        );
    }
    if let Some(baseline) = baseline {
        let last = curve.last().map_or(first, |segment| segment.end);
        builder.line_to(last.0, baseline);
        builder.line_to(first.0, baseline);
        builder.close();
    }
    builder.to_path()
}

fn unix_seconds(time: SystemTime) -> f64 {
    time.duration_since(UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64())
}

fn with_alpha(color: &gdk::RGBA, alpha: f32) -> gdk::RGBA {
    let mut color = *color;
    color.set_alpha(color.alpha() * alpha);
    color
}

/// Data and appearance needed for drawing.
pub struct ChartStyle<'a> {
    pub palette: &'a Palette,
    pub foreground: gdk::RGBA,
    pub unit_style: RateUnitStyle,
    pub compact: bool,
    pub pango: &'a pango::Context,
}

/// Draws the whole chart. `now` is the right end of the time axis.
pub fn draw(
    snapshot: &gtk::Snapshot,
    width: f32,
    height: f32,
    samples: &[TrafficSample],
    now: SystemTime,
    style: &ChartStyle,
) {
    let end = unix_seconds(now);
    let start = end - HISTORY_WINDOW.as_secs_f64();
    let visible: Vec<_> = samples
        .iter()
        .filter(|s| unix_seconds(s.timestamp) >= start)
        .collect();
    let peak = visible
        .iter()
        .map(|s| s.in_bps.max(s.out_bps))
        .fold(0.0, f64::max);
    // Ticks are whole numbers in the display unit (e.g. 0.5 / 1 / 1.5 / 2 MB/s).
    let axis = rate_axis(peak, style.unit_style);

    let layout = |text: &str, scale: f64| {
        let layout = pango::Layout::new(style.pango);
        let mut font = style.pango.font_description().unwrap_or_default();
        font.set_size((f64::from(font.size()) * scale) as i32);
        layout.set_font_description(Some(&font));
        layout.set_text(text);
        layout
    };
    let label_color = with_alpha(&style.foreground, 0.55);
    let grid_color = with_alpha(&style.foreground, 0.08);

    // Layout: vertical axis ticks on the left, times at the bottom (omitted when compact).
    let tick_labels: Vec<_> = axis
        .ticks
        .iter()
        .map(|&t| layout(&format_rate(t, style.unit_style), 0.8))
        .collect();
    let label_width = tick_labels
        .iter()
        .map(|l| l.pixel_size().0)
        .max()
        .unwrap_or(0) as f32;
    let label_height = tick_labels.first().map_or(0, |l| l.pixel_size().1) as f32;
    let bottom = if style.compact {
        0.0
    } else {
        label_height + 4.0
    };
    let plot = Plot {
        x: label_width + 8.0,
        y: label_height / 2.0,
        width: (width - label_width - 8.0 - 4.0).max(1.0),
        height: (height - label_height / 2.0 - bottom - label_height / 2.0).max(1.0),
        start,
        end,
        maximum: axis.maximum,
    };

    // Horizontal grid lines and vertical axis ticks
    for (tick, label) in axis.ticks.iter().zip(&tick_labels) {
        let y = plot.y_for(*tick);
        snapshot.append_color(
            &grid_color,
            &graphene::Rect::new(plot.x, y - 0.5, plot.width, 1.0),
        );
        let (w, h) = label.pixel_size();
        snapshot.save();
        snapshot.translate(&graphene::Point::new(
            plot.x - 8.0 - w as f32,
            y - h as f32 / 2.0,
        ));
        snapshot.append_layout(label, &label_color);
        snapshot.restore();
    }

    // Vertical grid lines and times every minute
    if !style.compact {
        for minute in minute_ticks(start, end) {
            let x = plot.x_for(minute);
            snapshot.append_color(
                &grid_color,
                &graphene::Rect::new(x - 0.5, plot.y, 1.0, plot.height),
            );
            let text = glib::DateTime::from_unix_local(minute as i64)
                .and_then(|t| t.format("%H:%M"))
                .map(|s| s.to_string())
                .unwrap_or_default();
            let label = layout(&text, 0.8);
            let (w, _) = label.pixel_size();
            let left = x - w as f32 / 2.0;
            if left >= plot.x && left + w as f32 <= plot.x + plot.width {
                snapshot.save();
                snapshot.translate(&graphene::Point::new(left, plot.y + plot.height + 4.0));
                snapshot.append_layout(&label, &label_color);
                snapshot.restore();
            }
        }
    }

    if visible.is_empty() {
        let label = layout(&gettext("Waiting for data…"), 1.0);
        let (w, h) = label.pixel_size();
        snapshot.save();
        snapshot.translate(&graphene::Point::new(
            plot.x + (plot.width - w as f32) / 2.0,
            plot.y + (plot.height - h as f32) / 2.0,
        ));
        snapshot.append_layout(&label, &label_color);
        snapshot.restore();
        return;
    }

    let plot_rect = graphene::Rect::new(plot.x, plot.y, plot.width, plot.height);
    snapshot.push_clip(&plot_rect);
    let series = [
        (
            style.palette.download,
            visible
                .iter()
                .map(|s| (s.timestamp, s.in_bps))
                .collect::<Vec<_>>(),
        ),
        (
            style.palette.upload,
            visible.iter().map(|s| (s.timestamp, s.out_bps)).collect(),
        ),
    ];
    for (color, values) in series {
        let points: Vec<_> = values
            .iter()
            .map(|&(t, v)| (plot.x_for(unix_seconds(t)), plot.y_for(v)))
            .collect();
        let Some(&first) = points.first() else {
            continue;
        };
        let curve = monotone_curve(&points);
        let line_path = curve_path(first, &curve, None);
        // Area: the same curve continued down to the baseline, filled with a gradient fading from top to bottom.
        let baseline = plot.y + plot.height;
        let area = curve_path(first, &curve, Some(baseline));
        snapshot.push_fill(&area, gsk::FillRule::Winding);
        snapshot.append_linear_gradient(
            &plot_rect,
            &graphene::Point::new(0.0, plot.y),
            &graphene::Point::new(0.0, baseline),
            &[
                gsk::ColorStop::new(0.0, with_alpha(&color, 0.35)),
                gsk::ColorStop::new(1.0, with_alpha(&color, 0.02)),
            ],
        );
        snapshot.pop();

        let stroke = gsk::Stroke::new(if style.compact { 1.5 } else { 2.0 });
        stroke.set_line_cap(gsk::LineCap::Round);
        stroke.set_line_join(gsk::LineJoin::Round);
        snapshot.push_shadow(&[gsk::Shadow::new(with_alpha(&color, 0.6), 0.0, 0.0, 4.0)]);
        snapshot.append_stroke(&line_path, &stroke, &color);
        snapshot.pop();
    }
    snapshot.pop();
}

mod imp {
    use super::*;

    #[derive(Debug, glib::Properties)]
    #[properties(wrapper_type = super::TrafficChart)]
    pub struct TrafficChart {
        /// Narrow layout: no time axis, thinner lines.
        #[property(get, set = Self::set_compact)]
        compact: Cell<bool>,
        pub(super) samples: RefCell<Vec<TrafficSample>>,
        pub(super) unit_style: Cell<RateUnitStyle>,
        pub(super) palette: Cell<Palette>,
        /// While visible, redraws every second so the time axis moves forward.
        pub(super) ticker: RefCell<Option<glib::SourceId>>,
    }

    impl Default for TrafficChart {
        fn default() -> Self {
            TrafficChart {
                compact: Cell::new(false),
                samples: RefCell::default(),
                unit_style: Cell::new(RateUnitStyle::Bits),
                palette: Cell::new(Palette::new(false, None, None)),
                ticker: RefCell::default(),
            }
        }
    }

    impl TrafficChart {
        fn set_compact(&self, compact: bool) {
            self.compact.set(compact);
            self.obj().queue_draw();
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for TrafficChart {
        const NAME: &'static str = "AnemobitTrafficChart";
        type Type = super::TrafficChart;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("anemobit-traffic-chart");
            klass.set_accessible_role(gtk::AccessibleRole::Img);
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for TrafficChart {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj()
                .update_property(&[gtk::accessible::Property::Label(&gettext(
                    "Traffic over the last 5 minutes",
                ))]);
        }
    }

    impl WidgetImpl for TrafficChart {
        fn measure(&self, orientation: gtk::Orientation, _for_size: i32) -> (i32, i32, i32, i32) {
            match orientation {
                gtk::Orientation::Horizontal => (160, 320, -1, -1),
                _ => (100, 180, -1, -1),
            }
        }

        fn map(&self) {
            self.parent_map();
            let weak = self.obj().downgrade();
            let id = glib::timeout_add_seconds_local(1, move || match weak.upgrade() {
                Some(chart) => {
                    chart.queue_draw();
                    glib::ControlFlow::Continue
                }
                None => glib::ControlFlow::Break,
            });
            if let Some(previous) = self.ticker.replace(Some(id)) {
                previous.remove();
            }
        }

        fn unmap(&self) {
            if let Some(id) = self.ticker.take() {
                id.remove();
            }
            self.parent_unmap();
        }

        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let widget = self.obj();
            let pango = widget.pango_context();
            let palette = self.palette.get();
            let style = ChartStyle {
                palette: &palette,
                foreground: widget.color(),
                unit_style: self.unit_style.get(),
                compact: self.compact.get(),
                pango: &pango,
            };
            draw(
                snapshot,
                widget.width() as f32,
                widget.height() as f32,
                &self.samples.borrow(),
                SystemTime::now(),
                &style,
            );
        }
    }
}

glib::wrapper! {
    pub struct TrafficChart(ObjectSubclass<imp::TrafficChart>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Default for TrafficChart {
    fn default() -> Self {
        glib::Object::new()
    }
}

impl TrafficChart {
    /// Recent samples (oldest to newest); anything older than 5 minutes isn't drawn.
    pub fn set_samples(&self, samples: impl IntoIterator<Item = TrafficSample>) {
        self.imp().samples.replace(samples.into_iter().collect());
        self.queue_draw();
    }

    pub fn set_unit_style(&self, style: RateUnitStyle) {
        self.imp().unit_style.set(style);
        self.queue_draw();
    }

    pub fn set_palette(&self, palette: &Palette) {
        self.imp().palette.set(*palette);
        self.queue_draw();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    #[test]
    fn two_points_make_a_straight_line() {
        let curve = monotone_curve(&[(0.0, 0.0), (3.0, 6.0)]);
        assert_eq!(curve.len(), 1);
        let s = curve[0];
        assert!(
            close(s.control1.0, 1.0) && close(s.control1.1, 2.0),
            "{s:?}"
        );
        assert!(
            close(s.control2.0, 2.0) && close(s.control2.1, 4.0),
            "{s:?}"
        );
        assert_eq!(s.end, (3.0, 6.0));
    }

    #[test]
    fn fewer_than_two_points_have_no_segments() {
        assert!(monotone_curve(&[]).is_empty());
        assert!(monotone_curve(&[(1.0, 1.0)]).is_empty());
    }

    #[test]
    fn curve_does_not_overshoot() {
        // Flat, then rising, then flat: control points must stay within each segment's y range.
        let points = [(0.0, 10.0), (1.0, 10.0), (2.0, 0.0), (3.0, 0.0), (4.0, 5.0)];
        let curve = monotone_curve(&points);
        assert_eq!(curve.len(), 4);
        for (i, segment) in curve.iter().enumerate() {
            let (y0, y1) = (points[i].1, points[i + 1].1);
            let (low, high) = (y0.min(y1), y0.max(y1));
            for y in [segment.control1.1, segment.control2.1] {
                assert!(
                    y >= low - 1e-4 && y <= high + 1e-4,
                    "segment {i}: {segment:?}"
                );
            }
            assert_eq!(segment.end, points[i + 1]);
        }
    }

    #[test]
    fn local_extremes_have_flat_tangents() {
        let curve = monotone_curve(&[(0.0, 0.0), (1.0, 5.0), (2.0, 0.0)]);
        // The tangent at the peak is horizontal: the incoming and outgoing control points both have y equal to the peak.
        assert!(close(curve[0].control2.1, 5.0));
        assert!(close(curve[1].control1.1, 5.0));
    }

    #[test]
    fn minute_ticks_inside_the_window() {
        assert_eq!(minute_ticks(100.0, 300.0), [120.0, 180.0, 240.0, 300.0]);
        assert_eq!(minute_ticks(120.0, 170.0), [120.0]);
        assert!(minute_ticks(121.0, 170.0).is_empty());
    }

    #[test]
    fn plot_maps_time_and_rate_to_pixels() {
        let plot = Plot {
            x: 10.0,
            y: 5.0,
            width: 300.0,
            height: 100.0,
            start: 0.0,
            end: 300.0,
            maximum: 1_000.0,
        };
        assert!(close(plot.x_for(0.0), 10.0));
        assert!(close(plot.x_for(150.0), 160.0));
        assert!(close(plot.x_for(300.0), 310.0));
        assert!(close(plot.y_for(0.0), 105.0));
        assert!(close(plot.y_for(1_000.0), 5.0));
        assert!(close(plot.y_for(250.0), 80.0));
    }
}
