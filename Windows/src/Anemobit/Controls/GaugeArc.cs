using System.Windows;
using System.Windows.Media;
using System.Windows.Media.Animation;

namespace Anemobit.Controls;

/// <summary>
/// One arc of the 270° ring gauge (clockwise from 135° at the bottom left). A <see cref="Fraction"/> of 1 is the whole track.
/// Value changes animate with a 0.6-second ease-out (same as the macOS version).
/// </summary>
public sealed class GaugeArc : FrameworkElement
{
    public static readonly DependencyProperty FractionProperty = DependencyProperty.Register(
        nameof(Fraction), typeof(double), typeof(GaugeArc), new PropertyMetadata(0.0, OnFractionChanged));

    public static readonly DependencyProperty AnimatesProperty = DependencyProperty.Register(
        nameof(Animates), typeof(bool), typeof(GaugeArc), new PropertyMetadata(false));

    public static readonly DependencyProperty StrokeThicknessProperty = DependencyProperty.Register(
        nameof(StrokeThickness), typeof(double), typeof(GaugeArc), new FrameworkPropertyMetadata(14.0, FrameworkPropertyMetadataOptions.AffectsRender));

    public static readonly DependencyProperty StrokeProperty = DependencyProperty.Register(
        nameof(Stroke), typeof(Brush), typeof(GaugeArc), new FrameworkPropertyMetadata(null, FrameworkPropertyMetadataOptions.AffectsRender));

    private static readonly DependencyProperty DisplayedFractionProperty = DependencyProperty.Register(
        "DisplayedFraction", typeof(double), typeof(GaugeArc), new FrameworkPropertyMetadata(0.0, FrameworkPropertyMetadataOptions.AffectsRender));

    private const double StartDegrees = 135;
    private const double SweepDegrees = 270;

    public double Fraction
    {
        get => (double)GetValue(FractionProperty);
        set => SetValue(FractionProperty, value);
    }

    /// <summary>The value arc animates; the track doesn't.</summary>
    public bool Animates
    {
        get => (bool)GetValue(AnimatesProperty);
        set => SetValue(AnimatesProperty, value);
    }

    public double StrokeThickness
    {
        get => (double)GetValue(StrokeThicknessProperty);
        set => SetValue(StrokeThicknessProperty, value);
    }

    public Brush? Stroke
    {
        get => (Brush?)GetValue(StrokeProperty);
        set => SetValue(StrokeProperty, value);
    }

    protected override void OnRender(DrawingContext drawingContext)
    {
        var fraction = Math.Clamp((double)GetValue(DisplayedFractionProperty), 0, 1);
        if (Stroke is null || fraction <= 0)
        {
            return;
        }

        var thickness = StrokeThickness;
        var radius = (Math.Min(ActualWidth, ActualHeight) - thickness) / 2;
        if (radius <= 0)
        {
            return;
        }
        var center = new Point(ActualWidth / 2, ActualHeight / 2);
        var sweep = Math.Max(SweepDegrees * fraction, 0.5);
        var start = PointOnCircle(center, radius, StartDegrees);
        var end = PointOnCircle(center, radius, StartDegrees + sweep);

        var geometry = new StreamGeometry();
        using (var context = geometry.Open())
        {
            context.BeginFigure(start, isFilled: false, isClosed: false);
            context.ArcTo(end, new Size(radius, radius), 0, sweep > 180, SweepDirection.Clockwise, isStroked: true, isSmoothJoin: false);
        }
        geometry.Freeze();

        var pen = new Pen(Stroke, thickness) { StartLineCap = PenLineCap.Round, EndLineCap = PenLineCap.Round };
        if (pen.CanFreeze)
        {
            pen.Freeze(); // Gradient pens with bindings can't be frozen
        }
        drawingContext.DrawGeometry(null, pen, geometry);
    }

    private static Point PointOnCircle(Point center, double radius, double degrees)
    {
        var radians = degrees * Math.PI / 180;
        return new Point(center.X + (radius * Math.Cos(radians)), center.Y + (radius * Math.Sin(radians)));
    }

    private static void OnFractionChanged(DependencyObject sender, DependencyPropertyChangedEventArgs e)
    {
        var arc = (GaugeArc)sender;
        var target = Math.Clamp((double)e.NewValue, 0, 1);
        if (!arc.Animates || !arc.IsLoaded)
        {
            arc.BeginAnimation(DisplayedFractionProperty, null);
            arc.SetValue(DisplayedFractionProperty, target);
            return;
        }
        var animation = new DoubleAnimation(target, TimeSpan.FromSeconds(0.6))
        {
            EasingFunction = new QuadraticEase { EasingMode = EasingMode.EaseOut },
        };
        arc.BeginAnimation(DisplayedFractionProperty, animation);
    }
}
