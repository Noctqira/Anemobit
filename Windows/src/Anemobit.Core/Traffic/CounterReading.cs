namespace Anemobit.Core.Traffic;

/// <summary>Raw counters from one SNMP sample.</summary>
/// <param name="InOctets">ifHCInOctets.</param>
/// <param name="OutOctets">ifHCOutOctets.</param>
/// <param name="SysUpTime">sysUpTime (hundredths of a second), helps detect whether the device restarted; null when unavailable.</param>
/// <param name="Timestamp">Sample time on the local monotonic clock (from an arbitrary origin).</param>
public readonly record struct CounterReading(ulong InOctets, ulong OutOctets, uint? SysUpTime, TimeSpan Timestamp);
