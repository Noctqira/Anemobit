namespace Anemobit.Core.Traffic;

/// <summary>Rate (bits per second).</summary>
public readonly record struct Rate(double InBps, double OutBps);
