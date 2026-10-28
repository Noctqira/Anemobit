using Anemobit.Core.Traffic;

namespace Anemobit.Core.Tests.Traffic;

public class BackoffTests
{
    [Fact]
    public void Next_Repeated_DoublesUpToMaximum()
    {
        var backoff = new Backoff(TimeSpan.FromSeconds(1), TimeSpan.FromSeconds(10));
        var delays = Enumerable.Range(0, 6).Select(_ => backoff.Next().TotalSeconds);
        Assert.Equal([1.0, 2, 4, 8, 10, 10], delays);
    }

    [Fact]
    public void Reset_AfterGrowth_StartsFromInitial()
    {
        var backoff = new Backoff(TimeSpan.FromSeconds(1), TimeSpan.FromSeconds(10));
        backoff.Next();
        backoff.Next();
        backoff.Reset();
        Assert.Equal(TimeSpan.FromSeconds(1), backoff.Next());
    }
}
