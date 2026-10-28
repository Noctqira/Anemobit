using Anemobit.Core.Ber;

namespace Anemobit.Core.Tests.Ber;

public class BerEncodingTests
{
    [Theory]
    [InlineData(0, new byte[] { 0x00 })]
    [InlineData(127, new byte[] { 0x7F })]
    public void EncodeLength_BelowOneHundredTwentyEight_UsesShortForm(int length, byte[] expected)
    {
        Assert.Equal(expected, BerEncoding.EncodeLength(length));
    }

    [Theory]
    [InlineData(128, new byte[] { 0x81, 0x80 })]
    [InlineData(200, new byte[] { 0x81, 0xC8 })]
    [InlineData(300, new byte[] { 0x82, 0x01, 0x2C })]
    [InlineData(70_000, new byte[] { 0x83, 0x01, 0x11, 0x70 })]
    public void EncodeLength_OneHundredTwentyEightOrMore_UsesLongForm(int length, byte[] expected)
    {
        Assert.Equal(expected, BerEncoding.EncodeLength(length));
    }

    [Fact]
    public void EncodeLength_Negative_ThrowsArgumentOutOfRange()
    {
        Assert.Throws<ArgumentOutOfRangeException>(() => BerEncoding.EncodeLength(-1));
    }

    [Fact]
    public void EncodeTlv_Sequence_WrapsContentWithTagAndLength()
    {
        byte[] content = [0x02, 0x01, 0x05, 0x05, 0x00];
        Assert.Equal([0x30, 0x05, .. content], BerEncoding.EncodeTlv(BerTag.Sequence, content));
    }
}
