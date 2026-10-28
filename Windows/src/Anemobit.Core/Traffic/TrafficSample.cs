namespace Anemobit.Core.Traffic;

/// <summary>One rate sample.</summary>
/// <param name="Timestamp">Wall-clock time for the chart's x axis (rate calculation itself uses the monotonic clock).</param>
/// <param name="InBps">Download rate (bps).</param>
/// <param name="OutBps">Upload rate (bps).</param>
/// <param name="DeviceUptime">Device uptime (sysUpTime); null when unavailable.</param>
public sealed record TrafficSample(DateTimeOffset Timestamp, double InBps, double OutBps, TimeSpan? DeviceUptime = null);
