namespace Anemobit.Core.Traffic;

/// <summary>Events yielded in order by <see cref="ITrafficPoller.RunAsync"/>: a status change or a new sample result.</summary>
public abstract record PollerUpdate
{
    private PollerUpdate()
    {
    }

    public sealed record StatusChanged(PollerStatus Status) : PollerUpdate;

    public sealed record SnapshotReceived(TrafficSnapshot Snapshot) : PollerUpdate;
}
