using Anemobit.Core.Snmp;

namespace Anemobit.Core.Tests.Snmp;

public class OidTests
{
    [Fact]
    public void TryParse_DottedString_ReturnsArcs()
    {
        Assert.True(Oid.TryParse("1.3.6.1.2.1.1.3.0", out var oid));
        Assert.Equal([1u, 3, 6, 1, 2, 1, 1, 3, 0], oid.Arcs);
    }

    [Fact]
    public void TryParse_LeadingDot_IsIgnored()
    {
        Assert.True(Oid.TryParse(".1.3.6", out var oid));
        Assert.Equal([1u, 3, 6], oid.Arcs);
    }

    [Theory]
    [InlineData(null)]
    [InlineData("")]
    [InlineData(".")]
    [InlineData("1..3")]
    [InlineData("1.3.a")]
    [InlineData("1.3.")]
    [InlineData("1.3.4294967296")]
    [InlineData("-1.3")]
    [InlineData("+1.3")]
    [InlineData("1. 3")]
    [InlineData("１.3")]
    public void TryParse_InvalidText_ReturnsFalse(string? text)
    {
        Assert.False(Oid.TryParse(text, out var oid));
        Assert.Null(oid);
    }

    [Fact]
    public void ToString_ParsedOid_ReturnsOriginalText()
    {
        const string Text = "1.3.6.1.2.1.31.1.1.1.6";
        Assert.True(Oid.TryParse(Text, out var oid));
        Assert.Equal(Text, oid.ToString());
    }

    [Fact]
    public void Appending_Arc_ReturnsChildWithArcAtEnd()
    {
        var root = new Oid(1, 3, 6);
        Assert.Equal(new Oid(1, 3, 6, 7), root.Appending(7));
        Assert.Equal(new Oid(1, 3, 6), root);
    }

    [Fact]
    public void IsPrefixOf_ChildAndUnrelated_DetectsPrefix()
    {
        var root = SnmpOids.IfName;
        var child = root.Appending(7);
        Assert.True(root.IsPrefixOf(child));
        Assert.True(root.IsPrefixOf(root));
        Assert.False(child.IsPrefixOf(root));
        Assert.False(new Oid(1, 3, 6, 2).IsPrefixOf(child));
    }

    [Fact]
    public void SuffixAfter_Prefix_ReturnsRemainingArcs()
    {
        var root = SnmpOids.IfName;
        Assert.Equal([7u], root.Appending(7).SuffixAfter(root)!);
        Assert.Null(root.SuffixAfter(root.Appending(7)));
    }

    [Fact]
    public void ComparisonOperators_CompareArcsLexicographically()
    {
        Assert.True(new Oid(1, 3, 6) < new Oid(1, 3, 6, 1));
        Assert.True(new Oid(1, 3, 6, 1) < new Oid(1, 3, 7));
        Assert.False(new Oid(1, 3, 7) < new Oid(1, 3, 6, 9));
        Assert.True(new Oid(1, 3, 7) >= new Oid(1, 3, 7));
        Assert.Equal(0, new Oid(1, 3).CompareTo(new Oid(1, 3)));
    }

    [Fact]
    public void Equals_SameArcs_AreEqualWithSameHashCode()
    {
        Assert.True(Oid.TryParse("1.3.6", out var parsed));
        Assert.Equal(new Oid(1, 3, 6), parsed);
        Assert.True(new Oid(1, 3, 6) == new Oid(1, 3, 6));
        Assert.Equal(new Oid(1, 3, 6).GetHashCode(), parsed.GetHashCode());
        Assert.NotEqual(new Oid(1, 3, 6), new Oid(1, 3, 6, 0));
    }

    [Fact]
    public void Constructor_SourceArrayMutatedAfterwards_DoesNotChangeOid()
    {
        uint[] source = [1, 3, 6];
        var oid = new Oid(source);
        source[2] = 9;
        Assert.Equal(new Oid(1, 3, 6), oid);
    }
}
