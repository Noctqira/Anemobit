namespace Anemobit.Core.Traffic;

/// <summary>Result of one sample: all monitored interfaces share the same time point and device uptime.</summary>
/// <param name="Timestamp">Wall-clock time for the chart's x axis.</param>
/// <param name="DeviceUptime">Device uptime (sysUpTime); null when unavailable.</param>
/// <param name="Samples">
/// Interface name (ifName) → that interface's rate sample. Interfaces with no rate this round (first sample, counter reset) are absent.
/// </param>
public sealed record TrafficSnapshot(
    DateTimeOffset Timestamp,
    TimeSpan? DeviceUptime,
    IReadOnlyDictionary<string, TrafficSample> Samples);
