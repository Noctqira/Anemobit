using System.Collections;
using System.Globalization;
using System.Windows;
using System.Windows.Data;

namespace Anemobit.Converters;

/// <summary>
/// Visible when "has a value": true, a non-empty string, a non-empty collection, or a non-null object. Reversed when <see cref="Invert"/> is true.
/// </summary>
public sealed class VisibilityConverter : IValueConverter
{
    public bool Invert { get; set; }

    public object Convert(object? value, Type targetType, object? parameter, CultureInfo culture)
    {
        var visible = value switch
        {
            bool flag => flag,
            string text => text.Length > 0,
            ICollection collection => collection.Count > 0,
            _ => value is not null,
        };
        return visible != Invert ? Visibility.Visible : Visibility.Collapsed;
    }

    public object ConvertBack(object? value, Type targetType, object? parameter, CultureInfo culture) =>
        throw new NotSupportedException();
}
