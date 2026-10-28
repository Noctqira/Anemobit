namespace Anemobit.Core.Formatting;

/// <summary>Chart y axis: maximum and ticks, all in bps.</summary>
public sealed record RateAxis(double Maximum, IReadOnlyList<double> Ticks);
