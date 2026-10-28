namespace Anemobit.Core.Traffic;

/// <summary>Exponential backoff: initial, 2×, 4×…, capped at maximum.</summary>
public sealed class Backoff(TimeSpan initial, TimeSpan maximum)
{
    private TimeSpan? _current;

    public TimeSpan Initial { get; } = initial;

    public TimeSpan Maximum { get; } = maximum;

    public TimeSpan Next()
    {
        var value = _current is { } current ? TimeSpan.FromTicks(Math.Min(current.Ticks * 2, Maximum.Ticks)) : Initial;
        _current = value;
        return value;
    }

    public void Reset() => _current = null;
}
