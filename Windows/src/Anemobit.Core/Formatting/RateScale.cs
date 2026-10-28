namespace Anemobit.Core.Formatting;

/// <summary>Gauge maximum and chart ticks.</summary>
public static class RateScale
{
    public const double Minimum = 1_000;

    /// <summary>Rounds up to the 1-2-5 series (1, 2, 5, 10, 20, 50…), at least 1 Kbps. In bits.</summary>
    public static double NiceCeiling(double bitsPerSecond)
    {
        if (!double.IsFinite(bitsPerSecond) || bitsPerSecond <= Minimum)
        {
            return Minimum;
        }
        var (step, magnitude) = NiceStep(bitsPerSecond);
        return step * magnitude;
    }

    /// <summary>
    /// Axis rounded in the display unit: the maximum and ticks are round numbers in that unit.
    /// E.g. a byte-mode peak of 3.7 MB/s → maximum 5 MB/s, a tick every 1 MB/s.
    /// </summary>
    public static RateAxis Axis(double peakBitsPerSecond, RateUnitStyle style)
    {
        var unitBase = style == RateUnitStyle.Bytes ? 1_024.0 : 1_000.0;
        var bitsPerAmount = style == RateUnitStyle.Bytes ? 8.0 : 1.0;

        // The peak in the display unit (starting at KB/s or Kbps).
        var unit = unitBase;
        var amount = (double.IsFinite(peakBitsPerSecond) ? Math.Max(peakBitsPerSecond, 0) : 0) / bitsPerAmount / unit;
        while (amount >= 1_000)
        {
            unit *= unitBase;
            amount /= unitBase;
        }

        var (step, magnitude) = NiceStep(Math.Max(amount, 1));
        var maximum = step * magnitude;
        // 1 → two steps, 2 → four steps, 5 → five steps, so ticks land on round numbers.
        var divisions = step switch
        {
            1 => 2,
            2 => 4,
            _ => 5,
        };
        var ticks = Enumerable.Range(0, divisions + 1)
            .Select(i => maximum * i / divisions * unit * bitsPerAmount)
            .ToArray();
        return new RateAxis(maximum * unit * bitsPerAmount, ticks);
    }

    /// <summary>Returns (1, 2 or 5, a power of 10) whose product is the smallest 1-2-5 number ≥ value.</summary>
    private static (double Step, double Magnitude) NiceStep(double value)
    {
        var magnitude = Math.Pow(10, Math.Floor(Math.Log10(value)));
        var mantissa = value / magnitude;
        foreach (var step in (ReadOnlySpan<double>)[1, 2, 5])
        {
            if (mantissa <= step * (1 + 1e-9))
            {
                return (step, magnitude);
            }
        }
        return (1, magnitude * 10);
    }
}
