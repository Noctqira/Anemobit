namespace Anemobit.Core.Traffic;

/// <summary>Traffic source (currently SNMP).</summary>
public interface ITrafficPoller
{
    /// <summary>
    /// Starts polling and yields status and sample results in order; on failure it backs off and retries automatically, never finishing.
    /// Polling stops and the connection closes when the consumer stops enumerating (<c>break</c>) or <paramref name="cancellationToken"/> is cancelled.
    /// </summary>
    IAsyncEnumerable<PollerUpdate> RunAsync(CancellationToken cancellationToken = default);
}
