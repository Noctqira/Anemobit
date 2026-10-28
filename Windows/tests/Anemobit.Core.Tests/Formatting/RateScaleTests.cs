using Anemobit.Core.Formatting;

namespace Anemobit.Core.Tests.Formatting;

public class RateScaleTests
{
    private const double MegabytePerSecond = 1_048_576.0 * 8; // 1 MB/s (binary) in bps

    [Theory]
    [InlineData(0.0, 1_000.0)]
    [InlineData(-5.0, 1_000.0)]
    [InlineData(500.0, 1_000.0)]
    [InlineData(1_000.0, 1_000.0)]
    [InlineData(1_001.0, 2_000.0)]
    [InlineData(1_200_000.0, 2_000_000.0)]
    [InlineData(2_000_000.0, 2_000_000.0)]
    [InlineData(3_700_000.0, 5_000_000.0)]
    [InlineData(37_000_000.0, 50_000_000.0)]
    [InlineData(51_000_000.0, 100_000_000.0)]
    [InlineData(999_000_000.0, 1_000_000_000.0)]
    [InlineData(double.NaN, 1_000.0)]
    [InlineData(double.PositiveInfinity, 1_000.0)]
    public void NiceCeiling_Value_RoundsUpToOneTwoFiveSequence(double value, double expected)
    {
        Assert.Equal(expected, RateScale.NiceCeiling(value));
    }

    [Fact]
    public void Axis_BitsFiveStep_UsesFiveDivisions()
    {
        var axis = RateScale.Axis(37_000_000, RateUnitStyle.Bits);
        Assert.Equal(50_000_000, axis.Maximum);
        Assert.Equal([0, 10_000_000, 20_000_000, 30_000_000, 40_000_000, 50_000_000], axis.Ticks);
    }

    [Fact]
    public void Axis_BitsTwoStep_UsesFourDivisions()
    {
        var axis = RateScale.Axis(1_200_000, RateUnitStyle.Bits);
        Assert.Equal(2_000_000, axis.Maximum);
        Assert.Equal([0, 500_000, 1_000_000, 1_500_000, 2_000_000], axis.Ticks);
    }

    [Fact]
    public void Axis_BitsOneStep_UsesTwoDivisions()
    {
        var axis = RateScale.Axis(800_000, RateUnitStyle.Bits);
        Assert.Equal(1_000_000, axis.Maximum);
        Assert.Equal([0, 500_000, 1_000_000], axis.Ticks);
    }

    [Fact]
    public void Axis_BytesExactPeak_RoundsInMegabytes()
    {
        var axis = RateScale.Axis(2 * MegabytePerSecond, RateUnitStyle.Bytes);
        Assert.Equal(2 * MegabytePerSecond, axis.Maximum);
        Assert.Equal(
            ["0 B/s", "512.0 KB/s", "1.0 MB/s", "1.5 MB/s", "2.0 MB/s"],
            axis.Ticks.Select(t => RateFormatter.Format(t, RateUnitStyle.Bytes)));
    }

    [Fact]
    public void Axis_BytesFractionalPeak_HasRoundLabels()
    {
        var axis = RateScale.Axis(3.7 * MegabytePerSecond, RateUnitStyle.Bytes);
        Assert.Equal(
            ["0 B/s", "1.0 MB/s", "2.0 MB/s", "3.0 MB/s", "4.0 MB/s", "5.0 MB/s"],
            axis.Ticks.Select(t => RateFormatter.Format(t, RateUnitStyle.Bytes)));
    }

    [Fact]
    public void Axis_ZeroOrNaN_UsesMinimum()
    {
        Assert.Equal(1_000, RateScale.Axis(0, RateUnitStyle.Bits).Maximum);
        Assert.Equal(8_192, RateScale.Axis(0, RateUnitStyle.Bytes).Maximum); // 1 KB/s
        Assert.Equal(8_192, RateScale.Axis(double.NaN, RateUnitStyle.Bytes).Maximum);
    }
}
