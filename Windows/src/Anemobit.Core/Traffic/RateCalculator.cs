namespace Anemobit.Core.Traffic;

/// <summary>
/// Calculates rates from two consecutive counter readings: difference × 8 ÷ actual elapsed time.
/// <list type="bullet">
/// <item>The first reading has no rate.</item>
/// <item>Counter or sysUpTime decreased (restart / reset) → discard and use the new value as the baseline, with no spike.</item>
/// <item>Elapsed time ≤ 0 → ignore the reading; the baseline is unchanged.</item>
/// </list>
/// </summary>
public sealed class RateCalculator
{
    private CounterReading? _previous;

    public Rate? Add(CounterReading reading)
    {
        if (_previous is not { } previous)
        {
            _previous = reading;
            return null;
        }

        var seconds = (reading.Timestamp - previous.Timestamp).TotalSeconds;
        if (seconds <= 0)
        {
            return null;
        }
        _previous = reading;

        if (previous.SysUpTime is { } oldUpTime && reading.SysUpTime is { } newUpTime && newUpTime < oldUpTime)
        {
            return null;
        }
        if (reading.InOctets < previous.InOctets || reading.OutOctets < previous.OutOctets)
        {
            return null;
        }

        var deltaIn = reading.InOctets - previous.InOctets;
        var deltaOut = reading.OutOctets - previous.OutOctets;
        return new Rate(deltaIn * 8.0 / seconds, deltaOut * 8.0 / seconds);
    }

    public void Reset() => _previous = null;
}
