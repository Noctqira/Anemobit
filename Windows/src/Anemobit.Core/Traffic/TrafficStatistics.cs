namespace Anemobit.Core.Traffic;

/// <summary>Average and peak (bps) over a set of samples.</summary>
public sealed record TrafficStatistics(double AverageIn, double AverageOut, double PeakIn, double PeakOut)
{
    /// <summary>Returns null when there are no samples.</summary>
    public static TrafficStatistics? From(IReadOnlyCollection<TrafficSample> samples)
    {
        ArgumentNullException.ThrowIfNull(samples);
        if (samples.Count == 0)
        {
            return null;
        }
        return new TrafficStatistics(
            samples.Average(s => s.InBps),
            samples.Average(s => s.OutBps),
            samples.Max(s => s.InBps),
            samples.Max(s => s.OutBps));
    }
}
