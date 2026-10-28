namespace Anemobit.Core.Tests.Fakes;

/// <summary>
/// Virtual clock: each timer (<c>Task.Delay(…, timeProvider)</c>) advances virtual time immediately when created,
/// then fires after a real 1 ms wait (to yield), and records each wait duration. Tests don't need to advance time manually.
/// </summary>
internal sealed class FakeTimeProvider : TimeProvider
{
    private static readonly DateTimeOffset Epoch = new(2026, 1, 1, 0, 0, 0, TimeSpan.Zero);
    private readonly Lock _lock = new();
    private readonly List<TimeSpan> _sleeps = [];
    private long _ticks;

    public IReadOnlyList<TimeSpan> Sleeps
    {
        get
        {
            lock (_lock)
            {
                return [.. _sleeps];
            }
        }
    }

    public override long TimestampFrequency => TimeSpan.TicksPerSecond;

    public override long GetTimestamp()
    {
        lock (_lock)
        {
            return _ticks;
        }
    }

    public override DateTimeOffset GetUtcNow() => Epoch.AddTicks(GetTimestamp());

    public override ITimer CreateTimer(TimerCallback callback, object? state, TimeSpan dueTime, TimeSpan period)
    {
        var timer = new FakeTimer(this, callback, state);
        timer.Change(dueTime, period);
        return timer;
    }

    private void Advance(TimeSpan duration)
    {
        lock (_lock)
        {
            _ticks += duration.Ticks;
            _sleeps.Add(duration);
        }
    }

    private sealed class FakeTimer(FakeTimeProvider owner, TimerCallback callback, object? state) : ITimer
    {
        private int _generation;
        private bool _disposed;

        public bool Change(TimeSpan dueTime, TimeSpan period)
        {
            var generation = Interlocked.Increment(ref _generation);
            if (dueTime == Timeout.InfiniteTimeSpan || _disposed)
            {
                return true;
            }
            owner.Advance(dueTime);
            _ = Task.Delay(1).ContinueWith(
                _ =>
                {
                    if (!_disposed && Volatile.Read(ref _generation) == generation)
                    {
                        callback(state);
                    }
                },
                TaskScheduler.Default);
            return true;
        }

        public void Dispose() => _disposed = true;

        public ValueTask DisposeAsync()
        {
            Dispose();
            return ValueTask.CompletedTask;
        }
    }
}
