using System.Windows;
using System.Windows.Controls;
using System.Windows.Media;
using Anemobit.ViewModels;

namespace Anemobit.Controls;

/// <summary>Connection status pill (same as StatusPill on macOS).</summary>
public partial class StatusPill : UserControl
{
    public static readonly DependencyProperty KindProperty = DependencyProperty.Register(
        nameof(Kind), typeof(StatusKind), typeof(StatusPill), new PropertyMetadata(StatusKind.NotSet, (d, _) => ((StatusPill)d).UpdateColors()));

    public static readonly DependencyProperty TextProperty = DependencyProperty.Register(
        nameof(Text), typeof(string), typeof(StatusPill), new PropertyMetadata(string.Empty));

    public static readonly DependencyProperty DetailProperty = DependencyProperty.Register(
        nameof(Detail), typeof(string), typeof(StatusPill), new PropertyMetadata(null));

    /// <summary>Color for OK (the theme's accent color).</summary>
    public static readonly DependencyProperty OkColorProperty = DependencyProperty.Register(
        nameof(OkColor), typeof(Color), typeof(StatusPill), new PropertyMetadata(Colors.LimeGreen, (d, _) => ((StatusPill)d).UpdateColors()));

    private static readonly DependencyPropertyKey DotColorKey = DependencyProperty.RegisterReadOnly(
        nameof(DotColor), typeof(Color), typeof(StatusPill), new PropertyMetadata(Colors.Gray));

    private static readonly DependencyPropertyKey DotBrushKey = DependencyProperty.RegisterReadOnly(
        nameof(DotBrush), typeof(Brush), typeof(StatusPill), new PropertyMetadata(Brushes.Gray));

    private static readonly DependencyPropertyKey FillBrushKey = DependencyProperty.RegisterReadOnly(
        nameof(FillBrush), typeof(Brush), typeof(StatusPill), new PropertyMetadata(Brushes.Transparent));

    public StatusPill()
    {
        InitializeComponent();
        UpdateColors();
    }

    public StatusKind Kind
    {
        get => (StatusKind)GetValue(KindProperty);
        set => SetValue(KindProperty, value);
    }

    public string Text
    {
        get => (string)GetValue(TextProperty);
        set => SetValue(TextProperty, value);
    }

    /// <summary>Tooltip (the error message on error).</summary>
    public string? Detail
    {
        get => (string?)GetValue(DetailProperty);
        set => SetValue(DetailProperty, value);
    }

    public Color OkColor
    {
        get => (Color)GetValue(OkColorProperty);
        set => SetValue(OkColorProperty, value);
    }

    public Color DotColor => (Color)GetValue(DotColorKey.DependencyProperty);

    public Brush DotBrush => (Brush)GetValue(DotBrushKey.DependencyProperty);

    public Brush FillBrush => (Brush)GetValue(FillBrushKey.DependencyProperty);

    private void UpdateColors()
    {
        var color = Kind switch
        {
            StatusKind.Ok => OkColor,
            StatusKind.Connecting => Color.FromRgb(0xE8, 0xB3, 0x0B),
            StatusKind.Failed => Color.FromRgb(0xE5, 0x48, 0x4D),
            _ => Color.FromRgb(0x8E, 0x8E, 0x93),
        };
        SetValue(DotColorKey, color);
        SetValue(DotBrushKey, Frozen(new SolidColorBrush(color)));
        SetValue(FillBrushKey, Frozen(new SolidColorBrush(Color.FromArgb(0x26, color.R, color.G, color.B))));
    }

    private static SolidColorBrush Frozen(SolidColorBrush brush)
    {
        brush.Freeze();
        return brush;
    }
}
