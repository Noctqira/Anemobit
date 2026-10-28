using Anemobit.Core.Traffic;

namespace Anemobit.Core.Tests.Traffic;

public class RollingCollectionTests
{
    [Fact]
    public void Add_BelowCapacity_KeepsInsertionOrder()
    {
        var buffer = new RollingCollection<int>(3) { 1, 2 };
        Assert.Equal([1, 2], buffer);
        Assert.Equal(2, buffer.Count);
        Assert.Equal(2, buffer.Last);
    }

    [Fact]
    public void Add_BeyondCapacity_DropsOldest()
    {
        var buffer = new RollingCollection<int>(3);
        for (var i = 1; i <= 7; i++)
        {
            buffer.Add(i);
        }
        Assert.Equal([5, 6, 7], buffer);
        Assert.Equal(3, buffer.Count);
        Assert.Equal(7, buffer.Last);
    }

    [Fact]
    public void Clear_FullBuffer_EmptiesAndAcceptsNewItems()
    {
        var buffer = new RollingCollection<int>(2) { 1, 2, 3 };
        buffer.Clear();
        Assert.Empty(buffer);
        Assert.Equal(default, buffer.Last);
        buffer.Add(9);
        Assert.Equal([9], buffer);
    }

    [Fact]
    public void Constructor_NonPositiveCapacity_Throws()
    {
        Assert.Throws<ArgumentOutOfRangeException>(() => new RollingCollection<int>(0));
    }
}
