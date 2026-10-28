using Anemobit.Core.Snmp;

namespace Anemobit.Core.Traffic;

/// <summary>SNMP polling settings. <see cref="object.ToString"/> excludes sensitive data (handled by <see cref="SnmpSecurity"/>).</summary>
public sealed record PollerConfiguration
{
    public required string Host { get; init; }

    public int Port { get; init; } = 161;

    public required SnmpSecurity Security { get; init; }

    /// <summary>Interface names (ifName) to monitor, resolved to ifIndex on every connection. Duplicate names keep only the first.</summary>
    public required IReadOnlyList<string> InterfaceNames
    {
        get;
        init => field = [.. value.Distinct(StringComparer.Ordinal)];
    }

    public TimeSpan Interval { get; init; } = TimeSpan.FromSeconds(1);

    public TimeSpan Timeout { get; init; } = SnmpClient.DefaultTimeout;

    public int Retries { get; init; } = SnmpClient.DefaultRetries;
}
