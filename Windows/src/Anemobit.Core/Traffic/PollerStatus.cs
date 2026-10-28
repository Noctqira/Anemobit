namespace Anemobit.Core.Traffic;

/// <summary>The poller's connection status.</summary>
public abstract record PollerStatus
{
    private PollerStatus()
    {
    }

    public sealed record Connecting : PollerStatus;

    public sealed record Connected : PollerStatus;

    /// <summary>Failed; retries automatically after <paramref name="RetryIn"/>. <paramref name="Error"/> is usually a <see cref="Snmp.SnmpException"/>.</summary>
    public sealed record Failed(Exception Error, TimeSpan RetryIn) : PollerStatus;
}
