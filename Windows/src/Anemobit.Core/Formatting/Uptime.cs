namespace Anemobit.Core.Formatting;

/// <summary>
/// Splits uptime into at most two units (days, hours, minutes; rounded down; units with a value of 0 are omitted).
/// The app composes the text ("12 days, 5 hours" / "12天5小時") for the locale.
/// </summary>
public static class Uptime
{
    public static IReadOnlyList<UptimePart> Parts(TimeSpan duration)
    {
        if (duration < TimeSpan.Zero)
        {
            duration = TimeSpan.Zero;
        }

        UptimePart[] all =
        [
            new(duration.Days, UptimeUnit.Days),
            new(duration.Hours, UptimeUnit.Hours),
            new(duration.Minutes, UptimeUnit.Minutes),
        ];
        var nonZero = all.Where(p => p.Value > 0).Take(2).ToArray();
        return nonZero.Length > 0 ? nonZero : [new UptimePart(0, UptimeUnit.Minutes)];
    }

    /// <summary>SNMP TimeTicks (hundredths of a second).</summary>
    public static TimeSpan FromTimeTicks(uint timeTicks) => TimeSpan.FromMilliseconds(timeTicks * 10.0);
}
