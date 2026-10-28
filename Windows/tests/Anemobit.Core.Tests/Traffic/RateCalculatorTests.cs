using Anemobit.Core.Traffic;

namespace Anemobit.Core.Tests.Traffic;

public class RateCalculatorTests
{
    [Fact]
    public void Add_FirstReading_ReturnsNull()
    {
        var calculator = new RateCalculator();
        Assert.Null(calculator.Add(Reading(1000, 1000, 0)));
    }

    [Fact]
    public void Add_OneSecondApart_ReturnsBitsPerSecond()
    {
        var calculator = new RateCalculator();
        calculator.Add(Reading(1_000_000, 500_000, 0));
        Assert.Equal(new Rate(10_000_000, 1_000_000), calculator.Add(Reading(2_250_000, 625_000, 1000)));
    }

    [Fact]
    public void Add_FractionalElapsedTime_DividesByActualElapsedTime()
    {
        var calculator = new RateCalculator();
        calculator.Add(Reading(0, 0, 0));

        var first = calculator.Add(Reading(300_000, 30_000, 1500));
        Assert.NotNull(first);
        Assert.Equal(1_600_000, first.Value.InBps, 3);
        Assert.Equal(160_000, first.Value.OutBps, 3);

        var second = calculator.Add(Reading(400_000, 30_000, 2300));
        Assert.NotNull(second);
        Assert.Equal(1_000_000, second.Value.InBps, 3); // 100_000 bytes × 8 / 0.8 s
        Assert.Equal(0, second.Value.OutBps);
    }

    [Fact]
    public void Add_StaticCounters_ReturnsZero()
    {
        var calculator = new RateCalculator();
        calculator.Add(Reading(42, 42, 0));
        Assert.Equal(new Rate(0, 0), calculator.Add(Reading(42, 42, 1000)));
    }

    [Fact]
    public void Add_CounterDecreased_DiscardsAndRebases()
    {
        var calculator = new RateCalculator();
        calculator.Add(Reading(10_000_000, 5_000_000, 0));
        // Device restarted: counter decreased
        Assert.Null(calculator.Add(Reading(1_000, 500, 1000)));
        // The next reading uses the reset value as the baseline
        Assert.Equal(new Rate(1_000_000, 0), calculator.Add(Reading(126_000, 500, 2000)));
    }

    [Fact]
    public void Add_OneDirectionDecreased_Discards()
    {
        var calculator = new RateCalculator();
        calculator.Add(Reading(10_000, 10_000, 0));
        Assert.Null(calculator.Add(Reading(20_000, 5_000, 1000)));
    }

    [Fact]
    public void Add_SysUpTimeDecreased_TreatsAsReboot()
    {
        var calculator = new RateCalculator();
        calculator.Add(Reading(1_000, 1_000, 0, upTime: 900_000));
        // The counter seems to increase, but sysUpTime decreased → restarted; the difference can't be trusted
        Assert.Null(calculator.Add(Reading(5_000_000, 5_000_000, 1000, upTime: 300)));
        Assert.Equal(new Rate(1_000_000, 0), calculator.Add(Reading(5_125_000, 5_000_000, 2000, upTime: 400)));
    }

    [Fact]
    public void Add_ZeroElapsedTime_IsIgnoredAndKeepsBaseline()
    {
        var calculator = new RateCalculator();
        calculator.Add(Reading(0, 0, 0));
        Assert.Null(calculator.Add(Reading(1_000, 1_000, 0)));
        Assert.Equal(new Rate(1_000_000, 0), calculator.Add(Reading(125_000, 0, 1000)));
    }

    [Fact]
    public void Add_CountersNearMaximum_DoNotOverflow()
    {
        var calculator = new RateCalculator();
        calculator.Add(Reading(ulong.MaxValue - 1_000, 1ul << 63, 0));
        Assert.Equal(new Rate(8_000, 1_000), calculator.Add(Reading(ulong.MaxValue, (1ul << 63) + 125, 1000)));
    }

    [Fact]
    public void Reset_AfterReading_ForgetsBaseline()
    {
        var calculator = new RateCalculator();
        calculator.Add(Reading(0, 0, 0));
        calculator.Reset();
        Assert.Null(calculator.Add(Reading(1_000, 1_000, 1000)));
    }

    private static CounterReading Reading(ulong inOctets, ulong outOctets, int milliseconds, uint? upTime = null) =>
        new(inOctets, outOctets, upTime, TimeSpan.FromMilliseconds(milliseconds));
}
