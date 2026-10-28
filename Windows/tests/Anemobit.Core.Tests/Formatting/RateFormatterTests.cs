using Anemobit.Core.Formatting;
using Anemobit.Core.Traffic;

namespace Anemobit.Core.Tests.Formatting;

public class RateFormatterTests
{
    [Theory]
    [InlineData(0.0, "0 bps")]
    [InlineData(999.0, "999 bps")]
    [InlineData(999.6, "1.0 Kbps")]
    [InlineData(1_000.0, "1.0 Kbps")]
    [InlineData(12_340.0, "12.3 Kbps")]
    [InlineData(999_949.0, "999.9 Kbps")]
    [InlineData(999_950.0, "1.0 Mbps")]
    [InlineData(12_345_678.0, "12.3 Mbps")]
    [InlineData(1_200_000.0, "1.2 Mbps")]
    [InlineData(950_000_000.0, "950.0 Mbps")]
    [InlineData(1_000_000_000.0, "1.0 Gbps")]
    [InlineData(2_500_000_000.0, "2.5 Gbps")]
    [InlineData(12_000_000_000_000.0, "12000.0 Gbps")]
    public void Format_Bits_UsesDecimalUnits(double bps, string expected)
    {
        Assert.Equal(expected, RateFormatter.Format(bps));
    }

    [Theory]
    [InlineData(-5.0)]
    [InlineData(double.NaN)]
    [InlineData(double.PositiveInfinity)]
    public void Format_NegativeOrNonFinite_ReturnsZero(double bps)
    {
        Assert.Equal("0 bps", RateFormatter.Format(bps));
    }

    [Theory]
    [InlineData(0.0, "0 B/s")]
    [InlineData(7_992.0, "999 B/s")]
    [InlineData(8_000.0, "1.0 KB/s")]          // 1000 B/s ≈ 0.98 KB/s
    [InlineData(8_192.0, "1.0 KB/s")]          // 1024 B/s
    [InlineData(8_191_508.0, "999.9 KB/s")]
    [InlineData(8_388_528.0, "1.0 MB/s")]      // 1023.99 KB/s rounds up
    [InlineData(103_179_878.4, "12.3 MB/s")]   // 12.3 × 1024² B/s
    [InlineData(21_474_836_480.0, "2.5 GB/s")] // 2.5 × 1024³ B/s
    public void Format_Bytes_UsesBinaryUnits(double bps, string expected)
    {
        Assert.Equal(expected, RateFormatter.Format(bps, RateUnitStyle.Bytes));
    }

    [Theory]
    [InlineData(0.0, "0", "bps")]
    [InlineData(999.0, "999", "bps")]
    [InlineData(12_345_678.0, "12.3", "Mbps")]
    [InlineData(999_950.0, "1.0", "Mbps")]
    [InlineData(2_500_000_000.0, "2.5", "Gbps")]
    public void Split_Bits_SeparatesValueAndUnit(double bps, string value, string unit)
    {
        Assert.Equal((value, unit), RateFormatter.Split(bps));
    }

    [Fact]
    public void Format_DecimalSeparator_IsAlwaysPeriod()
    {
        var previous = Thread.CurrentThread.CurrentCulture;
        try
        {
            Thread.CurrentThread.CurrentCulture = new System.Globalization.CultureInfo("de-DE");
            Assert.Equal("12.3 Mbps", RateFormatter.Format(12_345_678));
        }
        finally
        {
            Thread.CurrentThread.CurrentCulture = previous;
        }
    }

    [Fact]
    public void Summary_Bits_ShowsBothDirections()
    {
        var sample = new TrafficSample(DateTimeOffset.UnixEpoch, 12_345_678, 1_234_567);
        Assert.Equal("↓ 12.3 Mbps ↑ 1.2 Mbps", RateFormatter.Summary(sample));
    }

    [Fact]
    public void Summary_Bytes_ShowsBothDirections()
    {
        var sample = new TrafficSample(DateTimeOffset.UnixEpoch, 103_179_878.4, 8_192);
        Assert.Equal("↓ 12.3 MB/s ↑ 1.0 KB/s", RateFormatter.Summary(sample, RateUnitStyle.Bytes));
    }

    [Fact]
    public void AllUnits_EachStyle_ListsSmallestToLargest()
    {
        Assert.Equal(["bps", "Kbps", "Mbps", "Gbps"], RateFormatter.AllUnits(RateUnitStyle.Bits));
        Assert.Equal(["B/s", "KB/s", "MB/s", "GB/s"], RateFormatter.AllUnits(RateUnitStyle.Bytes));
    }
}
