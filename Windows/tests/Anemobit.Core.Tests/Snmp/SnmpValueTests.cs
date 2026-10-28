using Anemobit.Core.Ber;
using Anemobit.Core.Snmp;

namespace Anemobit.Core.Tests.Snmp;

public class SnmpValueTests
{
    public static TheoryData<SnmpValue, byte[]> ExceptionValues => new()
    {
        { new SnmpValue.NoSuchObject(), [0x80, 0x00] },
        { new SnmpValue.NoSuchInstance(), [0x81, 0x00] },
        { new SnmpValue.EndOfMibView(), [0x82, 0x00] },
    };

    // INTEGER

    [Theory]
    [InlineData(0L, new byte[] { 0x02, 0x01, 0x00 })]
    [InlineData(127L, new byte[] { 0x02, 0x01, 0x7F })]
    [InlineData(128L, new byte[] { 0x02, 0x02, 0x00, 0x80 })]
    [InlineData(256L, new byte[] { 0x02, 0x02, 0x01, 0x00 })]
    [InlineData(-1L, new byte[] { 0x02, 0x01, 0xFF })]
    [InlineData(-128L, new byte[] { 0x02, 0x01, 0x80 })]
    [InlineData(-129L, new byte[] { 0x02, 0x02, 0xFF, 0x7F })]
    [InlineData((long)int.MaxValue, new byte[] { 0x02, 0x04, 0x7F, 0xFF, 0xFF, 0xFF })]
    [InlineData((long)int.MinValue, new byte[] { 0x02, 0x04, 0x80, 0x00, 0x00, 0x00 })]
    public void Integer32_EncodeAndDecode_UsesShortestTwosComplement(long value, byte[] bytes)
    {
        AssertRoundTrip(new SnmpValue.Integer32(value), bytes);
    }

    [Fact]
    public void Decode_EmptyInteger_ThrowsInvalidContent()
    {
        Assert.Equal(BerError.InvalidContent, DecodeError(0x02, 0x00));
    }

    [Fact]
    public void Decode_IntegerWiderThan64Bits_ThrowsIntegerOverflow()
    {
        Assert.Equal(BerError.IntegerOverflow, DecodeError(0x02, 0x09, 0x01, 0, 0, 0, 0, 0, 0, 0, 0));
    }

    // OCTET STRING

    [Fact]
    public void OctetString_EncodeAndDecode_RoundTrips()
    {
        AssertRoundTrip(new SnmpValue.OctetString([.. "public"u8]), [0x04, 0x06, 0x70, 0x75, 0x62, 0x6C, 0x69, 0x63]);
    }

    [Fact]
    public void OctetString_Empty_RoundTrips()
    {
        AssertRoundTrip(new SnmpValue.OctetString([]), [0x04, 0x00]);
    }

    [Fact]
    public void OctetString_LongerThan127Bytes_UsesLongFormLength()
    {
        var value = new SnmpValue.OctetString([.. Enumerable.Repeat((byte)0x41, 200)]);
        var encoded = value.Encode();
        Assert.Equal([0x04, 0x81, 0xC8], encoded[..3]);
        Assert.Equal(203, encoded.Length);
        Assert.Equal(value, DecodeSingle(encoded));
    }

    [Fact]
    public void OctetString_300Bytes_UsesTwoLengthBytes()
    {
        var value = new SnmpValue.OctetString([.. Enumerable.Range(0, 300).Select(i => (byte)(i & 0xFF))]);
        var encoded = value.Encode();
        Assert.Equal([0x04, 0x82, 0x01, 0x2C], encoded[..4]);
        Assert.Equal(value, DecodeSingle(encoded));
    }

    // NULL, IpAddress

    [Fact]
    public void Null_EncodeAndDecode_RoundTrips()
    {
        AssertRoundTrip(new SnmpValue.Null(), [0x05, 0x00]);
    }

    [Fact]
    public void Decode_NullWithContent_ThrowsInvalidContent()
    {
        Assert.Equal(BerError.InvalidContent, DecodeError(0x05, 0x01, 0x00));
    }

    [Fact]
    public void IpAddress_EncodeAndDecode_RoundTrips()
    {
        AssertRoundTrip(new SnmpValue.IpAddress([192, 168, 88, 1]), [0x40, 0x04, 192, 168, 88, 1]);
    }

    [Fact]
    public void Decode_IpAddressNotFourBytes_ThrowsInvalidContent()
    {
        Assert.Equal(BerError.InvalidContent, DecodeError(0x40, 0x03, 192, 168, 88));
    }

    // Unsigned integers

    [Theory]
    [InlineData(0u, new byte[] { 0x41, 0x01, 0x00 })]
    [InlineData(127u, new byte[] { 0x41, 0x01, 0x7F })]
    [InlineData(128u, new byte[] { 0x41, 0x02, 0x00, 0x80 })]
    [InlineData(uint.MaxValue, new byte[] { 0x41, 0x05, 0x00, 0xFF, 0xFF, 0xFF, 0xFF })]
    public void Counter32_EncodeAndDecode_PadsHighBitWithZero(uint value, byte[] bytes)
    {
        AssertRoundTrip(new SnmpValue.Counter32(value), bytes);
    }

    [Fact]
    public void Gauge32_EncodeAndDecode_RoundTrips()
    {
        AssertRoundTrip(new SnmpValue.Gauge32(1_000_000_000), [0x42, 0x04, 0x3B, 0x9A, 0xCA, 0x00]);
    }

    [Fact]
    public void TimeTicks_EncodeAndDecode_RoundTrips()
    {
        AssertRoundTrip(new SnmpValue.TimeTicks(123_456), [0x43, 0x03, 0x01, 0xE2, 0x40]);
    }

    [Theory]
    [InlineData(0ul, new byte[] { 0x46, 0x01, 0x00 })]
    [InlineData(1ul << 32, new byte[] { 0x46, 0x05, 0x01, 0x00, 0x00, 0x00, 0x00 })]
    [InlineData(ulong.MaxValue, new byte[] { 0x46, 0x09, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF })]
    public void Counter64_EncodeAndDecode_HandlesBoundaries(ulong value, byte[] bytes)
    {
        AssertRoundTrip(new SnmpValue.Counter64(value), bytes);
    }

    [Fact]
    public void Decode_Counter64WithoutLeadingZero_IsTreatedAsUnsigned()
    {
        // Some agents omit the 0x00 prefix; still treat a set high bit as unsigned.
        Assert.Equal(
            new SnmpValue.Counter64(ulong.MaxValue),
            DecodeSingle(0x46, 0x08, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF));
    }

    [Fact]
    public void Decode_Counter32Above32Bits_ThrowsIntegerOverflow()
    {
        Assert.Equal(BerError.IntegerOverflow, DecodeError(0x41, 0x05, 0x01, 0x00, 0x00, 0x00, 0x00));
    }

    [Fact]
    public void Decode_Counter64Above64Bits_ThrowsIntegerOverflow()
    {
        Assert.Equal(BerError.IntegerOverflow, DecodeError(0x46, 0x09, 0x01, 0, 0, 0, 0, 0, 0, 0, 0));
    }

    // OBJECT IDENTIFIER

    [Fact]
    public void ObjectId_SysUpTime_EncodesAndDecodes()
    {
        AssertRoundTrip(
            new SnmpValue.ObjectId(SnmpOids.SysUpTime),
            [0x06, 0x08, 0x2B, 0x06, 0x01, 0x02, 0x01, 0x01, 0x03, 0x00]);
    }

    [Theory]
    [InlineData(1u, 3u, 0x2B)] // 40 * 1 + 3 = 43
    [InlineData(0u, 0u, 0x00)]
    [InlineData(2u, 5u, 0x55)] // 40 * 2 + 5 = 85
    public void ObjectId_FirstTwoArcs_AreCombinedIntoOneByte(uint first, uint second, byte combined)
    {
        AssertRoundTrip(new SnmpValue.ObjectId(new Oid(first, second)), [0x06, 0x01, combined]);
    }

    [Fact]
    public void ObjectId_CombinedFirstArcAbove127_UsesMultipleBytes()
    {
        // X.690 example: 2.999.3 → 88 37 03
        AssertRoundTrip(new SnmpValue.ObjectId(new Oid(2, 999, 3)), [0x06, 0x03, 0x88, 0x37, 0x03]);
    }

    [Fact]
    public void ObjectId_MultiByteArcs_EncodeAsBase128()
    {
        // 2636 (Juniper enterprise) → 0x94 0x4C; 14988 (MikroTik) → 0xF5 0x0C
        AssertRoundTrip(
            new SnmpValue.ObjectId(new Oid(1, 3, 6, 1, 4, 1, 2636, 14988)),
            [0x06, 0x09, 0x2B, 0x06, 0x01, 0x04, 0x01, 0x94, 0x4C, 0xF5, 0x0C]);
    }

    [Fact]
    public void ObjectId_MaxUInt32Arc_RoundTrips()
    {
        AssertRoundTrip(new SnmpValue.ObjectId(new Oid(1, 3, uint.MaxValue)), [0x06, 0x06, 0x2B, 0x8F, 0xFF, 0xFF, 0xFF, 0x7F]);
    }

    [Fact]
    public void Decode_OidArcAbove32Bits_ThrowsInvalidOid()
    {
        // 2^32 → 90 80 80 80 00
        Assert.Equal(BerError.InvalidOid, DecodeError(0x06, 0x06, 0x2B, 0x90, 0x80, 0x80, 0x80, 0x00));
    }

    [Fact]
    public void Decode_UnterminatedOidArc_ThrowsInvalidOid()
    {
        Assert.Equal(BerError.InvalidOid, DecodeError(0x06, 0x02, 0x2B, 0x86));
    }

    [Fact]
    public void Decode_EmptyOid_ThrowsInvalidOid()
    {
        Assert.Equal(BerError.InvalidOid, DecodeError(0x06, 0x00));
    }

    // Varbind exception values and unknown tags

    [Theory]
    [MemberData(nameof(ExceptionValues))]
    public void ExceptionValue_EncodeAndDecode_RoundTripsAndIsException(SnmpValue value, byte[] bytes)
    {
        AssertRoundTrip(value, bytes);
        Assert.True(value.IsException);
    }

    [Fact]
    public void IsException_RegularValues_IsFalse()
    {
        Assert.False(new SnmpValue.Null().IsException);
        Assert.False(new SnmpValue.Counter64(1).IsException);
    }

    [Fact]
    public void Decode_UnknownTag_ThrowsUnknownTag()
    {
        Assert.Equal(BerError.UnknownTag, DecodeError(0x1F, 0x00));
    }

    // Value access and equality

    [Fact]
    public void UnsignedValue_NumericTypes_ReturnsValue()
    {
        Assert.Equal(5ul, new SnmpValue.Counter64(5).UnsignedValue);
        Assert.Equal(7ul, new SnmpValue.Counter32(7).UnsignedValue);
        Assert.Equal(8ul, new SnmpValue.Gauge32(8).UnsignedValue);
        Assert.Equal(9ul, new SnmpValue.TimeTicks(9).UnsignedValue);
        Assert.Equal(3ul, new SnmpValue.Integer32(3).UnsignedValue);
    }

    [Fact]
    public void UnsignedValue_NegativeOrNonNumeric_ReturnsNull()
    {
        Assert.Null(new SnmpValue.Integer32(-1).UnsignedValue);
        Assert.Null(new SnmpValue.Null().UnsignedValue);
    }

    [Fact]
    public void StringValue_OctetString_DecodesUtf8()
    {
        Assert.Equal("pppoe-out1", new SnmpValue.OctetString([.. "pppoe-out1"u8]).StringValue);
        Assert.Null(new SnmpValue.Integer32(1).StringValue);
    }

    [Fact]
    public void Equals_ByteValues_ComparesContent()
    {
        Assert.Equal(new SnmpValue.OctetString([1, 2]), new SnmpValue.OctetString([1, 2]));
        Assert.Equal(new SnmpValue.OctetString([1, 2]).GetHashCode(), new SnmpValue.OctetString([1, 2]).GetHashCode());
        Assert.NotEqual(new SnmpValue.OctetString([1, 2]), new SnmpValue.OctetString([1, 3]));
        Assert.NotEqual<SnmpValue>(new SnmpValue.OctetString([1, 2, 3, 4]), new SnmpValue.IpAddress([1, 2, 3, 4]));
    }

    private static void AssertRoundTrip(SnmpValue value, byte[] bytes)
    {
        Assert.Equal(bytes, value.Encode());
        Assert.Equal(value, DecodeSingle(bytes));
    }

    /// <summary>Decodes a single TLV and checks there are no extra bytes.</summary>
    private static SnmpValue DecodeSingle(params byte[] bytes)
    {
        var reader = new BerReader(bytes);
        var value = SnmpValue.Decode(reader);
        Assert.True(reader.IsAtEnd);
        return value;
    }

    private static BerError DecodeError(params byte[] bytes) =>
        Assert.Throws<BerException>(() => DecodeSingle(bytes)).Error;
}
