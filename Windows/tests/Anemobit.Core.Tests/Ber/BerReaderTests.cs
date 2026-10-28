using Anemobit.Core.Ber;

namespace Anemobit.Core.Tests.Ber;

public class BerReaderTests
{
    [Fact]
    public void ReadTlv_NonMinimalLongFormLength_IsAccepted()
    {
        // Some implementations use the long form for short lengths; the decoder must accept it.
        var reader = new BerReader([0x04, 0x81, 0x02, 0x61, 0x62]);
        var (tag, content) = reader.ReadTlv();
        Assert.Equal(BerTag.OctetString, tag);
        Assert.Equal([0x61, 0x62], content.ToArray());
        Assert.True(reader.IsAtEnd);
    }

    [Fact]
    public void ReadTlv_IndefiniteLength_ThrowsIndefiniteLength()
    {
        Assert.Equal(BerError.IndefiniteLength, ReadTlvError(0x04, 0x80, 0x00, 0x00));
    }

    [Theory]
    [InlineData(new byte[] { 0x04, 0x05, 0x61 })]
    [InlineData(new byte[] { 0x04, 0x82, 0x01 })]
    [InlineData(new byte[] { 0x04 })]
    [InlineData(new byte[] { })]
    public void ReadTlv_LengthBeyondBuffer_ThrowsTruncated(byte[] bytes)
    {
        Assert.Equal(BerError.Truncated, ReadTlvError(bytes));
    }

    [Fact]
    public void ReadTlv_LengthWiderThanFourBytes_ThrowsLengthTooLarge()
    {
        Assert.Equal(BerError.LengthTooLarge, ReadTlvError(0x04, 0x85, 0x00, 0x00, 0x00, 0x00, 0x01, 0x61));
    }

    [Fact]
    public void ReadTlv_FourByteLengthAboveIntMaxValue_ThrowsTruncated()
    {
        Assert.Equal(BerError.Truncated, ReadTlvError(0x04, 0x84, 0xFF, 0xFF, 0xFF, 0xFF, 0x61));
    }

    [Fact]
    public void ReadSequence_NestedValues_ReadsInOrderAndStopsAtEnd()
    {
        var reader = new BerReader([0x30, 0x05, 0x02, 0x01, 0x05, 0x05, 0x00, 0x05, 0x00]);
        var sequence = reader.ReadSequence();

        var (firstTag, firstContent) = sequence.ReadTlv();
        Assert.Equal(BerTag.Integer, firstTag);
        Assert.Equal([0x05], firstContent.ToArray());
        var (secondTag, secondContent) = sequence.ReadTlv();
        Assert.Equal(BerTag.Null, secondTag);
        Assert.True(secondContent.IsEmpty);
        Assert.True(sequence.IsAtEnd);
        Assert.False(reader.IsAtEnd);
        Assert.Equal(BerTag.Null, reader.ReadTlv().Tag);
        Assert.True(reader.IsAtEnd);
    }

    [Fact]
    public void ReadSequence_OtherTag_ThrowsUnexpectedTag()
    {
        var reader = new BerReader([0x04, 0x00]);
        var error = Assert.Throws<BerException>(() => reader.ReadSequence());
        Assert.Equal(BerError.UnexpectedTag, error.Error);
        Assert.Equal((byte)0x30, error.ExpectedTag);
        Assert.Equal((byte)0x04, error.Tag);
    }

    [Fact]
    public void PeekTag_WithData_DoesNotAdvance()
    {
        var reader = new BerReader([0x05, 0x00]);
        Assert.Equal(BerTag.Null, reader.PeekTag());
        Assert.Equal(BerTag.Null, reader.ReadTlv().Tag);
        Assert.True(reader.IsAtEnd);
    }

    [Fact]
    public void PeekTag_AtEnd_ThrowsTruncated()
    {
        var reader = new BerReader([]);
        Assert.Equal(BerError.Truncated, Assert.Throws<BerException>(() => reader.PeekTag()).Error);
    }

    private static BerError ReadTlvError(params byte[] bytes) =>
        Assert.Throws<BerException>(() => new BerReader(bytes).ReadTlv()).Error;
}
