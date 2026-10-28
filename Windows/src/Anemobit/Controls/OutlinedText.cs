using System.Globalization;
using System.Windows;
using System.Windows.Documents;
using System.Windows.Media;

namespace Anemobit.Controls;

/// <summary>
/// Outlined text (for the floating window): first draws a solid halo along the glyph outlines, then the text on top.
/// No blurred shadow: blur smears the glyph edges (especially in light mode), and the effect turns the whole text into a bitmap, which looks soft.
/// </summary>
public sealed class OutlinedText : FrameworkElement
{
    public static readonly DependencyProperty TextProperty = DependencyProperty.Register(
        nameof(Text), typeof(string), typeof(OutlinedText),
        new FrameworkPropertyMetadata(string.Empty, FrameworkPropertyMetadataOptions.AffectsMeasure | FrameworkPropertyMetadataOptions.AffectsRender));

    public static readonly DependencyProperty FillProperty = DependencyProperty.Register(
        nameof(Fill), typeof(Brush), typeof(OutlinedText), new FrameworkPropertyMetadata(Brushes.Black, FrameworkPropertyMetadataOptions.AffectsRender));

    public static readonly DependencyProperty HaloProperty = DependencyProperty.Register(
        nameof(Halo), typeof(Brush), typeof(OutlinedText), new FrameworkPropertyMetadata(null, FrameworkPropertyMetadataOptions.AffectsRender));

    /// <summary>Width of the halo outside the glyphs (DIP).</summary>
    public static readonly DependencyProperty HaloWidthProperty = DependencyProperty.Register(
        nameof(HaloWidth), typeof(double), typeof(OutlinedText),
        new FrameworkPropertyMetadata(1.0, FrameworkPropertyMetadataOptions.AffectsMeasure | FrameworkPropertyMetadataOptions.AffectsRender));

    public static readonly DependencyProperty FontFamilyProperty = TextElement.FontFamilyProperty.AddOwner(
        typeof(OutlinedText), new FrameworkPropertyMetadata(SystemFonts.MessageFontFamily, FrameworkPropertyMetadataOptions.Inherits | FrameworkPropertyMetadataOptions.AffectsMeasure | FrameworkPropertyMetadataOptions.AffectsRender));

    public static readonly DependencyProperty FontSizeProperty = TextElement.FontSizeProperty.AddOwner(
        typeof(OutlinedText), new FrameworkPropertyMetadata(SystemFonts.MessageFontSize, FrameworkPropertyMetadataOptions.Inherits | FrameworkPropertyMetadataOptions.AffectsMeasure | FrameworkPropertyMetadataOptions.AffectsRender));

    public static readonly DependencyProperty FontWeightProperty = TextElement.FontWeightProperty.AddOwner(
        typeof(OutlinedText), new FrameworkPropertyMetadata(FontWeights.Normal, FrameworkPropertyMetadataOptions.Inherits | FrameworkPropertyMetadataOptions.AffectsMeasure | FrameworkPropertyMetadataOptions.AffectsRender));

    public string Text
    {
        get => (string)GetValue(TextProperty);
        set => SetValue(TextProperty, value);
    }

    public Brush Fill
    {
        get => (Brush)GetValue(FillProperty);
        set => SetValue(FillProperty, value);
    }

    public Brush? Halo
    {
        get => (Brush?)GetValue(HaloProperty);
        set => SetValue(HaloProperty, value);
    }

    public double HaloWidth
    {
        get => (double)GetValue(HaloWidthProperty);
        set => SetValue(HaloWidthProperty, value);
    }

    public FontFamily FontFamily
    {
        get => (FontFamily)GetValue(FontFamilyProperty);
        set => SetValue(FontFamilyProperty, value);
    }

    public double FontSize
    {
        get => (double)GetValue(FontSizeProperty);
        set => SetValue(FontSizeProperty, value);
    }

    public FontWeight FontWeight
    {
        get => (FontWeight)GetValue(FontWeightProperty);
        set => SetValue(FontWeightProperty, value);
    }

    /// <summary>Measures text width in this element's font (for fixed column widths, excluding the halo).</summary>
    public double MeasureTextWidth(string text) => Format(text, Fill).WidthIncludingTrailingWhitespace;

    protected override Size MeasureOverride(Size availableSize)
    {
        var text = Format(Text, Fill);
        return new Size(text.WidthIncludingTrailingWhitespace + (HaloWidth * 2), text.Height + (HaloWidth * 2));
    }

    protected override void OnRender(DrawingContext drawingContext)
    {
        var origin = new Point(HaloWidth, HaloWidth);
        var text = Format(Text, Fill);
        if (Halo is not null && HaloWidth > 0)
        {
            // Half the stroke width falls outside the glyph, so use double the width; round joins keep corners from spiking.
            var pen = new Pen(Halo, HaloWidth * 2) { LineJoin = PenLineJoin.Round };
            pen.Freeze();
            drawingContext.DrawGeometry(Halo, pen, text.BuildGeometry(origin));
        }
        drawingContext.DrawText(text, origin);
    }

    private FormattedText Format(string text, Brush brush)
    {
        return new FormattedText(
            text ?? string.Empty,
            CultureInfo.CurrentCulture,
            FlowDirection.LeftToRight,
            new Typeface(FontFamily, FontStyles.Normal, FontWeight, FontStretches.Normal),
            FontSize,
            brush,
            VisualTreeHelper.GetDpi(this).PixelsPerDip);
    }
}
