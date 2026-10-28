using System.Globalization;
using System.Windows.Data;
using Anemobit.Core.Formatting;
using Anemobit.Core.Snmp;
using Anemobit.Resources;
using Anemobit.Settings;

namespace Anemobit.Converters;

/// <summary>
/// Display names for option rows and drop-downs. Security modes (None / Auth / Priv) and protocol names aren't translated;
/// refresh rates and rate units follow the UI language; language names are written in their own language.
/// </summary>
public sealed class DisplayNameConverter : IValueConverter
{
    public static DisplayNameConverter Instance { get; } = new();

    public object Convert(object? value, Type targetType, object? parameter, CultureInfo culture) => value switch
    {
        SecurityMode.None => "None",
        SecurityMode.Authorized => "Auth",
        SecurityMode.Private => "Priv",
        SnmpAuthProtocol auth => auth.ToString().ToUpperInvariant(),
        SnmpPrivProtocol priv => priv.ToString().ToUpperInvariant(),
        RateUnitStyle.Bits => Strings.UnitBits,
        RateUnitStyle.Bytes => Strings.UnitBytes,
        int seconds => string.Format(CultureInfo.CurrentCulture, Strings.SecondsFormat, seconds),
        AppLanguage language => language.NativeName() ?? Strings.FollowSystem,
        _ => value?.ToString() ?? string.Empty,
    };

    public object ConvertBack(object? value, Type targetType, object? parameter, CultureInfo culture) =>
        throw new NotSupportedException();
}
