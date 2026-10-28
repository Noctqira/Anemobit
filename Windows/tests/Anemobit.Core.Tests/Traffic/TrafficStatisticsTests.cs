using Anemobit.Core.Traffic;

namespace Anemobit.Core.Tests.Traffic;

public class TrafficStatisticsTests
{
    [Fact]
    public void From_NoSamples_ReturnsNull()
    {
        Assert.Null(TrafficStatistics.From([]));
    }

    [Fact]
    public void From_Samples_ComputesAverageAndPeak()
    {
        var stats = TrafficStatistics.From([Sample(10, 1), Sample(30, 5), Sample(20, 3)]);
        Assert.Equal(new TrafficStatistics(AverageIn: 20, AverageOut: 3, PeakIn: 30, PeakOut: 5), stats);
    }

    private static TrafficSample Sample(double inBps, double outBps) => new(DateTimeOffset.UnixEpoch, inBps, outBps);
}
