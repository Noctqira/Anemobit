using System.Globalization;
using System.Windows;
using System.Windows.Media;
using System.Windows.Threading;
using Anemobit.Core.Formatting;
using Anemobit.Core.Traffic;
using Anemobit.Resources;
using Anemobit.ViewModels;

namespace Anemobit.Controls;

/// <summary>
/// Area chart of download / upload over the last 5 minutes (monotone cubic curves, gradient fill). Y-axis ticks are whole numbers in the display unit;
/// redraws every second so the time axis scrolls forward (even without new samples).
/// </summary>
public sealed class TrafficChart : FrameworkElement
{
    public static readonly DependencyProperty TrafficProperty = DependencyProperty.Register(
        nameof(Traffic), typeof(InterfaceTraffic), typeof(TrafficChart), new FrameworkPropertyMetadata(null, FrameworkPropertyMetadataOptions.AffectsRender));

    public static readonly DependencyProperty RevisionProperty = DependencyProperty.Register(
        nameof(Revision), typeof(int), typeof(TrafficChart), new FrameworkPropertyMetadata(0, FrameworkPropertyMetadataOptions.AffectsRender));

    public static readonly DependencyProperty UnitStyleProperty = DependencyProperty.Register(
        nameof(UnitStyle), typeof(RateUnitStyle), typeof(TrafficChart), new FrameworkPropertyMetadata(RateUnitStyle.Bits, FrameworkPropertyMetadataOptions.AffectsRender));

    public static readonly DependencyProperty DownloadColorProperty = DependencyProperty.Register(
        nameof(DownloadColor), typeof(Color), typeof(TrafficChart), new FrameworkPropertyMetadata(Colors.DeepSkyBlue, FrameworkPropertyMetadataOptions.AffectsRender));

    public static readonly DependencyProperty UploadColorProperty = DependencyProperty.Register(
        nameof(UploadColor), typeof(Color), typeof(TrafficChart), new FrameworkPropertyMetadata(Colors.Orchid, FrameworkPropertyMetadataOptions.AffectsRender));

    public static readonly DependencyProperty GridBrushProperty = DependencyProperty.Register(
        nameof(GridBrush), typeof(Brush), typeof(TrafficChart), new FrameworkPropertyMetadata(Brushes.LightGray, FrameworkPropertyMetadataOptions.AffectsRender));

    public static readonly DependencyProperty LabelBrushProperty = DependencyProperty.Register(
        nameof(LabelBrush), typeof(Brush), typeof(TrafficChart), new FrameworkPropertyMetadata(Brushes.Gray, FrameworkPropertyMetadataOptions.AffectsRender));

    /// <summary>Compact version (notification area flyout): no time axis labels, thinner lines.</summary>
    public static readonly DependencyProperty CompactProperty = DependencyProperty.Register(
        nameof(Compact), typeof(bool), typeof(TrafficChart), new FrameworkPropertyMetadata(false, FrameworkPropertyMetadataOptions.AffectsRender));

    private const double LabelFontSize = 10;
    private static readonly Typeface LabelTypeface = new("Segoe UI Variable Text, Segoe UI");
    private readonly DispatcherTimer _timer = new() { Interval = TimeSpan.FromSeconds(1) };

    public TrafficChart()
    {
        _timer.Tick += (_, _) => InvalidateVisual();
        Loaded += (_, _) => _timer.Start();
        Unloaded += (_, _) => _timer.Stop();
    }

    public InterfaceTraffic? Traffic
    {
        get => (InterfaceTraffic?)GetValue(TrafficProperty);
        set => SetValue(TrafficProperty, value);
    }

    public int Revision
    {
        get => (int)GetValue(RevisionProperty);
        set => SetValue(RevisionProperty, value);
    }

    public RateUnitStyle UnitStyle
    {
        get => (RateUnitStyle)GetValue(UnitStyleProperty);
        set => SetValue(UnitStyleProperty, value);
    }

    public Color DownloadColor
    {
        get => (Color)GetValue(DownloadColorProperty);
        set => SetValue(DownloadColorProperty, value);
    }

    public Color UploadColor
    {
        get => (Color)GetValue(UploadColorProperty);
        set => SetValue(UploadColorProperty, value);
    }

    public Brush GridBrush
    {
        get => (Brush)GetValue(GridBrushProperty);
        set => SetValue(GridBrushProperty, value);
    }

    public Brush LabelBrush
    {
        get => (Brush)GetValue(LabelBrushProperty);
        set => SetValue(LabelBrushProperty, value);
    }

    public bool Compact
    {
        get => (bool)GetValue(CompactProperty);
        set => SetValue(CompactProperty, value);
    }

    protected override void OnRender(DrawingContext drawingContext)
    {
        var bounds = new Rect(0, 0, ActualWidth, ActualHeight);
        drawingContext.DrawRectangle(Brushes.Transparent, null, bounds); // Lets the whole area receive mouse events
        if (bounds.Width < 40 || bounds.Height < 30)
        {
            return;
        }

        var dpi = VisualTreeHelper.GetDpi(this).PixelsPerDip;
        var end = DateTimeOffset.UtcNow;
        var start = end - InterfaceTraffic.HistoryWindow;
        var visible = Traffic?.History.Where(s => s.Timestamp >= start).ToList() ?? [];
        var peak = visible.Count == 0 ? 0 : visible.Max(s => Math.Max(s.InBps, s.OutBps));
        var axis = RateScale.Axis(peak, UnitStyle);

        // Y-axis label width, x-axis label height
        var yLabels = axis.Ticks.Select(t => Label(RateFormatter.Format(t, UnitStyle), dpi)).ToList();
        var labelWidth = yLabels.Max(l => l.Width) + 6;
        var xLabelHeight = Compact ? 0 : Label("00:00", dpi).Height + 4;
        var plot = new Rect(labelWidth, 4, Math.Max(bounds.Width - labelWidth - 2, 1), Math.Max(bounds.Height - xLabelHeight - 6, 1));

        var gridPen = new Pen(GridBrush, 1);
        gridPen.Freeze();

        // Horizontal grid lines and y-axis ticks
        for (var i = 0; i < axis.Ticks.Count; i++)
        {
            var y = Snap(plot.Bottom - (axis.Ticks[i] / axis.Maximum * plot.Height));
            drawingContext.DrawLine(gridPen, new Point(plot.Left, y), new Point(plot.Right, y));
            var label = yLabels[i];
            drawingContext.DrawText(label, new Point(plot.Left - label.Width - 6, y - (label.Height / 2)));
        }

        // Vertical grid lines: every minute
        var firstMinute = new DateTimeOffset(start.Year, start.Month, start.Day, start.Hour, start.Minute, 0, start.Offset).AddMinutes(1);
        var lastLabelRight = double.NegativeInfinity;
        for (var minute = firstMinute; minute <= end; minute = minute.AddMinutes(1))
        {
            var x = Snap(XFor(minute, start, plot));
            drawingContext.DrawLine(gridPen, new Point(x, plot.Top), new Point(x, plot.Bottom));
            if (!Compact)
            {
                var label = Label(minute.ToLocalTime().ToString("t", CultureInfo.CurrentCulture), dpi);
                var labelX = Math.Clamp(x - (label.Width / 2), plot.Left, plot.Right - label.Width);
                if (labelX > lastLabelRight + 8)
                {
                    // Labels pushed in at the edges overlap their neighbors; overlapping labels aren't drawn.
                    drawingContext.DrawText(label, new Point(labelX, plot.Bottom + 4));
                    lastLabelRight = labelX + label.Width;
                }
            }
        }

        if (visible.Count == 0)
        {
            var waiting = new FormattedText(
                Strings.WaitingForData, CultureInfo.CurrentUICulture, FlowDirection.LeftToRight, LabelTypeface, 13, LabelBrush, dpi);
            drawingContext.DrawText(waiting, new Point(plot.Left + ((plot.Width - waiting.Width) / 2), plot.Top + ((plot.Height - waiting.Height) / 2)));
            return;
        }

        drawingContext.PushClip(new RectangleGeometry(plot));
        DrawSeries(drawingContext, visible, s => s.InBps, DownloadColor, start, axis.Maximum, plot);
        DrawSeries(drawingContext, visible, s => s.OutBps, UploadColor, start, axis.Maximum, plot);
        drawingContext.Pop();
    }

    private void DrawSeries(
        DrawingContext drawingContext, List<TrafficSample> samples, Func<TrafficSample, double> value, Color color,
        DateTimeOffset start, double maximum, Rect plot)
    {
        var points = samples
            .Select(s => new Point(XFor(s.Timestamp, start, plot), plot.Bottom - (Math.Clamp(value(s) / maximum, 0, 1) * plot.Height)))
            .ToList();

        var line = new StreamGeometry();
        using (var context = line.Open())
        {
            MonotoneCurve.Draw(context, points, startFigure: true, closeToBottom: null);
        }
        line.Freeze();

        var area = new StreamGeometry();
        using (var context = area.Open())
        {
            MonotoneCurve.Draw(context, points, startFigure: true, closeToBottom: plot.Bottom);
        }
        area.Freeze();

        var fill = new LinearGradientBrush(
            Color.FromArgb(0x59, color.R, color.G, color.B),
            Color.FromArgb(0x05, color.R, color.G, color.B),
            new Point(0, 0),
            new Point(0, 1));
        fill.Freeze();
        drawingContext.DrawGeometry(fill, null, area);

        var pen = new Pen(new SolidColorBrush(color), Compact ? 1.5 : 2) { LineJoin = PenLineJoin.Round, StartLineCap = PenLineCap.Round, EndLineCap = PenLineCap.Round };
        pen.Freeze();
        drawingContext.DrawGeometry(null, pen, line);
    }

    private static double XFor(DateTimeOffset time, DateTimeOffset start, Rect plot) =>
        plot.Left + ((time - start) / InterfaceTraffic.HistoryWindow * plot.Width);

    private static double Snap(double value) => Math.Round(value) + 0.5;

    private FormattedText Label(string text, double pixelsPerDip) =>
        new(text, CultureInfo.CurrentUICulture, FlowDirection.LeftToRight, LabelTypeface, LabelFontSize, LabelBrush, pixelsPerDip);
}
