using System.Globalization;
using System.Windows.Data;
using System.Windows.Media;
using Anemobit.Settings;

namespace Anemobit.Converters;

/// <summary>"#RRGGBB" ↔ brush (color swatches).</summary>
public sealed class HexColorConverter : IValueConverter
{
    public object Convert(object? value, Type targetType, object? parameter, CultureInfo culture)
    {
        var color = HexColor.ToColor(value as string) ?? Colors.Transparent;
        var brush = new SolidColorBrush(color);
        brush.Freeze();
        return brush;
    }

    public object ConvertBack(object? value, Type targetType, object? parameter, CultureInfo culture) =>
        throw new NotSupportedException();
}
