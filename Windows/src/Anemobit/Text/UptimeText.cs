using System.Globalization;
using Anemobit.Core.Formatting;
using Anemobit.Resources;

namespace Anemobit.Text;

/// <summary>Uptime → "12 days, 5 hours" / "12天5小時" (in the UI language).</summary>
public static class UptimeText
{
    public static string Format(TimeSpan duration, CultureInfo? culture = null)
    {
        culture ??= CultureInfo.CurrentUICulture;
        var separator = Strings.ResourceManager.GetString(nameof(Strings.UptimeSeparator), culture) ?? ", ";
        return string.Join(separator, Uptime.Parts(duration).Select(part => FormatPart(part, culture)));
    }

    private static string FormatPart(UptimePart part, CultureInfo culture)
    {
        var key = (part.Unit, part.Value == 1) switch
        {
            (UptimeUnit.Days, true) => nameof(Strings.UptimeDayOne),
            (UptimeUnit.Days, false) => nameof(Strings.UptimeDayOther),
            (UptimeUnit.Hours, true) => nameof(Strings.UptimeHourOne),
            (UptimeUnit.Hours, false) => nameof(Strings.UptimeHourOther),
            (UptimeUnit.Minutes, true) => nameof(Strings.UptimeMinuteOne),
            _ => nameof(Strings.UptimeMinuteOther),
        };
        var format = Strings.ResourceManager.GetString(key, culture) ?? "{0}";
        return string.Format(culture, format, part.Value);
    }
}
