using Anemobit.Core.Formatting;
using Anemobit.Core.Traffic;
using Anemobit.Tests.Fakes;
using Anemobit.ViewModels;

namespace Anemobit.Tests.ViewModels;

public class InterfaceTrafficTests
{
    private static readonly DateTimeOffset Now = new(2026, 10, 11, 12, 0, 0, TimeSpan.Zero);

    [Fact]
    public void NewInterface_NoData_ShowsPlaceholders()
    {
        using var culture = UiCulture.English();
        var traffic = Traffic();

        Assert.Equal("--", traffic.CurrentIn);
        Assert.Equal("--", traffic.InValue);
        Assert.Equal(0, traffic.InFraction);
        Assert.Equal("Max 1.0 Kbps", traffic.GaugeMaxInText);
        Assert.Equal("↓ … ↑ …", traffic.Summary);
    }

    [Fact]
    public void Append_Sample_UpdatesCurrentStatisticsAndGauge()
    {
        using var culture = UiCulture.English();
        var traffic = Traffic();

        traffic.Append(Sample(Now.AddSeconds(-2), 10_000_000, 1_000_000));
        traffic.Append(Sample(Now.AddSeconds(-1), 30_000_000, 3_000_000));

        Assert.Equal("30.0 Mbps", traffic.CurrentIn);
        Assert.Equal("20.0 Mbps", traffic.AverageIn);
        Assert.Equal("30.0 Mbps", traffic.PeakIn);
        Assert.Equal(("30.0", "Mbps"), (traffic.InValue, traffic.InUnit));
        Assert.Equal(50_000_000, traffic.GaugeMaxIn);
        Assert.Equal(0.6, traffic.InFraction, 6);
        Assert.Equal("Max 50.0 Mbps", traffic.GaugeMaxInText);
        Assert.Equal("↓ 30.0 Mbps ↑ 3.0 Mbps", traffic.Summary);
        Assert.Equal(2, traffic.Revision);
    }

    [Fact]
    public void Statistics_SamplesOlderThanFiveMinutes_AreIgnored()
    {
        using var culture = UiCulture.English();
        var traffic = Traffic();

        traffic.Append(Sample(Now.AddMinutes(-6), 90_000_000, 0));
        traffic.Append(Sample(Now.AddSeconds(-1), 10_000_000, 0));

        Assert.Equal("10.0 Mbps", traffic.PeakIn);
        Assert.Single(traffic.RecentSamples());
        Assert.Equal(2, traffic.History.Count);
    }

    [Fact]
    public void UnitStyle_ChangedToBytes_ReformatsAndNotifies()
    {
        using var culture = UiCulture.English();
        var traffic = Traffic();
        traffic.Append(Sample(Now, 8_388_608, 0));
        var changed = new List<string?>();
        traffic.PropertyChanged += (_, e) => changed.Add(e.PropertyName);

        traffic.UnitStyle = RateUnitStyle.Bytes;

        Assert.Equal("1.0 MB/s", traffic.CurrentIn);
        Assert.Contains(nameof(InterfaceTraffic.CurrentIn), changed);
        Assert.Contains(nameof(InterfaceTraffic.GaugeMaxIn), changed);
    }

    [Fact]
    public void ClearLatest_AfterData_KeepsHistory()
    {
        var traffic = Traffic();
        traffic.Append(Sample(Now, 1_000, 1_000));

        traffic.ClearLatest();

        Assert.Null(traffic.Latest);
        Assert.Equal("--", traffic.CurrentIn);
        Assert.Single(traffic.History);
    }

    [Fact]
    public void History_MoreThanCapacity_KeepsMostRecent()
    {
        var traffic = Traffic();
        for (var i = 0; i < InterfaceTraffic.HistoryCapacity + 10; i++)
        {
            traffic.Append(Sample(Now.AddSeconds(i - 400), i, 0));
        }

        Assert.Equal(InterfaceTraffic.HistoryCapacity, traffic.History.Count);
        Assert.Equal(10, traffic.History.First().InBps);
    }

    private static InterfaceTraffic Traffic() => new("pppoe-out1", RateUnitStyle.Bits, new FixedTimeProvider(Now));

    private static TrafficSample Sample(DateTimeOffset at, double inBps, double outBps) => new(at, inBps, outBps);
}
