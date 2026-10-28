namespace Anemobit.Core.Formatting;

/// <summary>One uptime unit and its value, e.g. (12, Days).</summary>
public readonly record struct UptimePart(int Value, UptimeUnit Unit);
