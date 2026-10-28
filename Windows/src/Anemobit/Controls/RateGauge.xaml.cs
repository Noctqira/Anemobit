using System.Windows;
using System.Windows.Controls;
using System.Windows.Media;

namespace Anemobit.Controls;

/// <summary>270° ring rate gauge (same as RateGauge on macOS). Designed at 200×200 and scaled proportionally to the available space.</summary>
public partial class RateGauge : UserControl
{
    public static readonly DependencyProperty TitleProperty = Register<string>(nameof(Title), string.Empty);
    public static readonly DependencyProperty GlyphProperty = Register<string>(nameof(Glyph), string.Empty);
    public static readonly DependencyProperty ValueProperty = Register<string>(nameof(Value), "--");
    public static readonly DependencyProperty UnitProperty = Register<string>(nameof(Unit), " ");
    public static readonly DependencyProperty MaxTextProperty = Register<string>(nameof(MaxText), string.Empty);
    public static readonly DependencyProperty FractionProperty = Register<double>(nameof(Fraction), 0.0);
    public static readonly DependencyProperty CompactProperty = DependencyProperty.Register(
        nameof(Compact), typeof(bool), typeof(RateGauge), new PropertyMetadata(false, OnCompactChanged));
    public static readonly DependencyProperty GlowOpacityProperty = Register<double>(nameof(GlowOpacity), 0.5);

    public static readonly DependencyProperty ColorProperty = DependencyProperty.Register(
        nameof(Color), typeof(Color), typeof(RateGauge), new PropertyMetadata(Colors.DeepSkyBlue, OnColorChanged));

    private static readonly DependencyPropertyKey FadedColorKey = DependencyProperty.RegisterReadOnly(
        nameof(FadedColor), typeof(Color), typeof(RateGauge), new PropertyMetadata(Colors.Transparent));

    private static readonly DependencyPropertyKey BrushKey = DependencyProperty.RegisterReadOnly(
        nameof(Brush), typeof(Brush), typeof(RateGauge), new PropertyMetadata(Brushes.DeepSkyBlue));

    private static readonly DependencyPropertyKey ArcThicknessKey = DependencyProperty.RegisterReadOnly(
        nameof(ArcThickness), typeof(double), typeof(RateGauge), new PropertyMetadata(14.0));

    private static readonly DependencyPropertyKey ValueFontSizeKey = DependencyProperty.RegisterReadOnly(
        nameof(ValueFontSize), typeof(double), typeof(RateGauge), new PropertyMetadata(38.0));

    public RateGauge()
    {
        InitializeComponent();
    }

    public string Title
    {
        get => (string)GetValue(TitleProperty);
        set => SetValue(TitleProperty, value);
    }

    /// <summary>Segoe Fluent Icons glyph (download / upload arrow).</summary>
    public string Glyph
    {
        get => (string)GetValue(GlyphProperty);
        set => SetValue(GlyphProperty, value);
    }

    public string Value
    {
        get => (string)GetValue(ValueProperty);
        set => SetValue(ValueProperty, value);
    }

    public string Unit
    {
        get => (string)GetValue(UnitProperty);
        set => SetValue(UnitProperty, value);
    }

    public string MaxText
    {
        get => (string)GetValue(MaxTextProperty);
        set => SetValue(MaxTextProperty, value);
    }

    public double Fraction
    {
        get => (double)GetValue(FractionProperty);
        set => SetValue(FractionProperty, value);
    }

    /// <summary>Narrow layouts use a thinner ring and smaller text.</summary>
    public bool Compact
    {
        get => (bool)GetValue(CompactProperty);
        set => SetValue(CompactProperty, value);
    }

    /// <summary>Glow strength (stronger in dark mode).</summary>
    public double GlowOpacity
    {
        get => (double)GetValue(GlowOpacityProperty);
        set => SetValue(GlowOpacityProperty, value);
    }

    public Color Color
    {
        get => (Color)GetValue(ColorProperty);
        set => SetValue(ColorProperty, value);
    }

    public Color FadedColor => (Color)GetValue(FadedColorKey.DependencyProperty);

    public Brush Brush => (Brush)GetValue(BrushKey.DependencyProperty);

    public double ArcThickness => (double)GetValue(ArcThicknessKey.DependencyProperty);

    public double ValueFontSize => (double)GetValue(ValueFontSizeKey.DependencyProperty);

    private static DependencyProperty Register<T>(string name, T defaultValue) =>
        DependencyProperty.Register(name, typeof(T), typeof(RateGauge), new PropertyMetadata(defaultValue));

    private static void OnCompactChanged(DependencyObject sender, DependencyPropertyChangedEventArgs e)
    {
        var compact = (bool)e.NewValue;
        sender.SetValue(ArcThicknessKey, compact ? 11.0 : 14.0);
        sender.SetValue(ValueFontSizeKey, compact ? 32.0 : 38.0);
    }

    private static void OnColorChanged(DependencyObject sender, DependencyPropertyChangedEventArgs e)
    {
        var gauge = (RateGauge)sender;
        var color = (Color)e.NewValue;
        gauge.SetValue(FadedColorKey, Color.FromArgb(0x73, color.R, color.G, color.B));
        var brush = new SolidColorBrush(color);
        brush.Freeze();
        gauge.SetValue(BrushKey, brush);
    }
}
