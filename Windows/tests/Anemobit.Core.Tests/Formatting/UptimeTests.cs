using Anemobit.Core.Formatting;

namespace Anemobit.Core.Tests.Formatting;

public class UptimeTests
{
    [Theory]
    [InlineData(0, new[] { 0, 0, 0 }, new[] { UptimeUnit.Minutes })]
    [InlineData(59, new[] { 0, 0, 0 }, new[] { UptimeUnit.Minutes })]
    [InlineData(3_660, new[] { 0, 1, 1 }, new[] { UptimeUnit.Hours, UptimeUnit.Minutes })]
    [InlineData(86_399, new[] { 0, 23, 59 }, new[] { UptimeUnit.Hours, UptimeUnit.Minutes })] // Rounded down
    [InlineData(86_400 * 12 + 3_600 * 5 + 120, new[] { 12, 5, 2 }, new[] { UptimeUnit.Days, UptimeUnit.Hours })]
    [InlineData(86_400 * 3 + 120, new[] { 3, 0, 2 }, new[] { UptimeUnit.Days, UptimeUnit.Minutes })]
    [InlineData(86_400 * 3, new[] { 3, 0, 0 }, new[] { UptimeUnit.Days })]
    public void Parts_Duration_ReturnsAtMostTwoNonZeroUnits(int seconds, int[] dayHourMinute, UptimeUnit[] expectedUnits)
    {
        var parts = Uptime.Parts(TimeSpan.FromSeconds(seconds));
        Assert.Equal(expectedUnits, parts.Select(p => p.Unit));
        Assert.All(parts, p => Assert.Equal(dayHourMinute[(int)p.Unit], p.Value));
    }

    [Fact]
    public void Parts_Negative_IsZeroMinutes()
    {
        Assert.Equal([new UptimePart(0, UptimeUnit.Minutes)], Uptime.Parts(TimeSpan.FromSeconds(-5)));
    }

    [Fact]
    public void FromTimeTicks_HundredthsOfSecond_ConvertsToTimeSpan()
    {
        Assert.Equal(TimeSpan.FromSeconds(3_660), Uptime.FromTimeTicks(366_000));
    }
}
