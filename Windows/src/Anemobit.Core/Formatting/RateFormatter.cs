using System.Globalization;
using Anemobit.Core.Traffic;

namespace Anemobit.Core.Formatting;

/// <summary>bps → "12.3 Mbps" or "1.5 MB/s". Units and the decimal point don't change with the locale.</summary>
public static class RateFormatter
{
    private static readonly Scale BitsScale = new(1_000, "bps", ["Kbps", "Mbps", "Gbps"]);
    private static readonly Scale BytesScale = new(1_024, "B/s", ["KB/s", "MB/s", "GB/s"]);

    /// <summary>All units this style can produce (for measuring fixed-width layouts).</summary>
    public static IReadOnlyList<string> AllUnits(RateUnitStyle style)
    {
        var scale = ScaleFor(style);
        return [scale.BaseUnit, .. scale.Units];
    }

    public static string Format(double bitsPerSecond, RateUnitStyle style = RateUnitStyle.Bits)
    {
        var (value, unit) = Split(bitsPerSecond, style);
        return $"{value} {unit}";
    }

    /// <summary>Value and unit separately, e.g. ("12.3", "Mbps"). The value is at most "999.9" (except in the largest unit).</summary>
    public static (string Value, string Unit) Split(double bitsPerSecond, RateUnitStyle style = RateUnitStyle.Bits)
    {
        var scale = ScaleFor(style);
        var amount = style == RateUnitStyle.Bytes ? bitsPerSecond / 8 : bitsPerSecond;
        if (!double.IsFinite(amount) || amount <= 0)
        {
            return ("0", scale.BaseUnit);
        }

        var whole = Math.Round(amount, MidpointRounding.AwayFromZero);
        if (whole < 1_000)
        {
            return (whole.ToString("0", CultureInfo.InvariantCulture), scale.BaseUnit);
        }

        // Round to one decimal first; if that reaches 1000, use the next unit, so the value never exceeds 4 integer digits.
        var divisor = 1.0;
        for (var i = 0; i < scale.Units.Length; i++)
        {
            divisor *= scale.Base;
            var scaled = Math.Round(amount / divisor * 10, MidpointRounding.AwayFromZero) / 10;
            if (scaled < 1_000 || i == scale.Units.Length - 1)
            {
                return (scaled.ToString("0.0", CultureInfo.InvariantCulture), scale.Units[i]);
            }
        }
        return ("0", scale.BaseUnit);
    }

    /// <summary>"↓ 12.3 Mbps ↑ 1.2 Mbps", for the floating window and notification area tooltip.</summary>
    public static string Summary(TrafficSample sample, RateUnitStyle style = RateUnitStyle.Bits)
    {
        ArgumentNullException.ThrowIfNull(sample);
        return $"↓ {Format(sample.InBps, style)} ↑ {Format(sample.OutBps, style)}";
    }

    private static Scale ScaleFor(RateUnitStyle style) => style == RateUnitStyle.Bytes ? BytesScale : BitsScale;

    private sealed record Scale(double Base, string BaseUnit, string[] Units);
}
