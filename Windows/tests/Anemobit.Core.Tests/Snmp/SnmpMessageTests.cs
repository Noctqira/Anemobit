using Anemobit.Core.Ber;
using Anemobit.Core.Snmp;
using Anemobit.Core.Tests.Fakes;

namespace Anemobit.Core.Tests.Snmp;

public class SnmpMessageTests
{
    [Fact]
    public void Encode_GetRequest_MatchesReferenceBytes()
    {
        var message = new SnmpMessage("public", Pdu.Get(1, SnmpOids.SysUpTime));
        byte[] expected =
        [
            0x30, 0x26,
            0x02, 0x01, 0x01,                                   // version = 1 (v2c)
            0x04, 0x06, 0x70, 0x75, 0x62, 0x6C, 0x69, 0x63,     // "public"
            0xA0, 0x19,                                         // GetRequest
            0x02, 0x01, 0x01,                                   // request-id
            0x02, 0x01, 0x00,                                   // error-status
            0x02, 0x01, 0x00,                                   // error-index
            0x30, 0x0E,
            0x30, 0x0C,
            0x06, 0x08, 0x2B, 0x06, 0x01, 0x02, 0x01, 0x01, 0x03, 0x00,
            0x05, 0x00,
        ];
        Assert.Equal(expected, message.Encode());
    }

    [Fact]
    public void Encode_GetNextRequest_UsesTagA1()
    {
        var message = new SnmpMessage("public", Pdu.GetNext(1, SnmpOids.SysUpTime));
        Assert.Equal(0xA1, message.Encode()[13]);
    }

    [Fact]
    public void Encode_GetBulkRequest_MatchesReferenceBytes()
    {
        var message = new SnmpMessage("public", Pdu.GetBulk(0x1234, nonRepeaters: 0, maxRepetitions: 10, SnmpOids.IfName));
        byte[] expected =
        [
            0x30, 0x29,
            0x02, 0x01, 0x01,
            0x04, 0x06, 0x70, 0x75, 0x62, 0x6C, 0x69, 0x63,
            0xA5, 0x1C,                                         // GetBulkRequest
            0x02, 0x02, 0x12, 0x34,                             // request-id
            0x02, 0x01, 0x00,                                   // non-repeaters
            0x02, 0x01, 0x0A,                                   // max-repetitions
            0x30, 0x10,
            0x30, 0x0E,
            0x06, 0x0A, 0x2B, 0x06, 0x01, 0x02, 0x01, 0x1F, 0x01, 0x01, 0x01, 0x01,
            0x05, 0x00,
        ];
        Assert.Equal(expected, message.Encode());
    }

    [Fact]
    public void Decode_RequestWithMultipleVarBinds_PreservesOrder()
    {
        Oid[] oids = [SnmpOids.IfHCInOctets.Appending(3), SnmpOids.IfHCOutOctets.Appending(3), SnmpOids.SysUpTime];
        var decoded = SnmpMessage.Decode(new SnmpMessage("c", Pdu.Get(7, oids)).Encode());
        Assert.Equal(oids, decoded.Pdu.VarBinds.Select(b => b.Oid));
        Assert.All(decoded.Pdu.VarBinds, b => Assert.Equal(new SnmpValue.Null(), b.Value));
    }

    [Fact]
    public void Decode_ResponseWithNoSuchInstance_ReturnsExceptionValue()
    {
        byte[] bytes =
        [
            0x30, 0x29,
            0x02, 0x01, 0x01,
            0x04, 0x06, 0x70, 0x75, 0x62, 0x6C, 0x69, 0x63,
            0xA2, 0x1C,                                         // GetResponse
            0x02, 0x01, 0x01,
            0x02, 0x01, 0x00,
            0x02, 0x01, 0x00,
            0x30, 0x11,
            0x30, 0x0F,
            0x06, 0x0B, 0x2B, 0x06, 0x01, 0x02, 0x01, 0x1F, 0x01, 0x01, 0x01, 0x06, 0x63,
            0x81, 0x00,                                         // noSuchInstance
        ];
        var message = SnmpMessage.Decode(bytes);
        Assert.Equal(SnmpMessage.VersionV2c, message.Version);
        Assert.Equal("public"u8.ToArray(), message.Community);
        Assert.Equal(PduType.Response, message.Pdu.Type);
        Assert.Equal(1, message.Pdu.RequestId);
        Assert.Equal(0, message.Pdu.ErrorStatus);
        Assert.Equal(
            [new VarBind(SnmpOids.IfHCInOctets.Appending(99), new SnmpValue.NoSuchInstance())],
            message.Pdu.VarBinds);
    }

    [Fact]
    public void Decode_ResponseWithErrorStatus_ExposesStatusAndIndex()
    {
        byte[] bytes =
        [
            0x30, 0x26,
            0x02, 0x01, 0x01,
            0x04, 0x06, 0x70, 0x75, 0x62, 0x6C, 0x69, 0x63,
            0xA2, 0x19,
            0x02, 0x01, 0x02,                                   // request-id 2
            0x02, 0x01, 0x02,                                   // error-status noSuchName
            0x02, 0x01, 0x01,                                   // error-index 1
            0x30, 0x0E,
            0x30, 0x0C,
            0x06, 0x08, 0x2B, 0x06, 0x01, 0x02, 0x01, 0x01, 0x03, 0x00,
            0x05, 0x00,
        ];
        var pdu = SnmpMessage.Decode(bytes).Pdu;
        Assert.Equal(2, pdu.RequestId);
        Assert.Equal(2, pdu.ErrorStatus);
        Assert.Equal(1, pdu.ErrorIndex);
        Assert.Equal(SnmpErrorStatus.NoSuchName, pdu.KnownErrorStatus);
    }

    [Theory]
    [InlineData(99L)]
    [InlineData(-1L)]
    [InlineData(1L << 40)]
    public void KnownErrorStatus_UndefinedValue_IsNull(long status)
    {
        Assert.Null(new Pdu(PduType.Response, 1, status, 0, []).KnownErrorStatus);
    }

    [Fact]
    public void Decode_EncodedResponse_RoundTrips()
    {
        var pdu = new Pdu(
            PduType.Response,
            RequestId: -5,
            ErrorStatus: 0,
            ErrorIndex: 0,
            VarBinds:
            [
                new VarBind(SnmpOids.IfHCInOctets.Appending(3), new SnmpValue.Counter64(ulong.MaxValue)),
                new VarBind(SnmpOids.SysUpTime, new SnmpValue.TimeTicks(4_294_967_295)),
                new VarBind(SnmpOids.IfName.Appending(3), FakeAgent.Octets("pppoe-out1")),
            ]);
        var original = new SnmpMessage("public", pdu);
        Assert.Equal(original, SnmpMessage.Decode(original.Encode()));
    }

    [Fact]
    public void Equals_SameContent_AreEqualWithSameHashCode()
    {
        var a = new SnmpMessage("public", Pdu.Get(1, SnmpOids.SysUpTime));
        var b = new SnmpMessage("public", Pdu.Get(1, SnmpOids.SysUpTime));
        Assert.Equal(a, b);
        Assert.Equal(a.GetHashCode(), b.GetHashCode());
        Assert.NotEqual(a, new SnmpMessage("private", Pdu.Get(1, SnmpOids.SysUpTime)));
        Assert.NotEqual(a, new SnmpMessage("public", Pdu.Get(1, SnmpOids.SysUpTime, SnmpOids.IfName)));
    }

    [Fact]
    public void Decode_SnmpV1Message_ThrowsUnsupportedVersion()
    {
        var bytes = EncodedGet();
        bytes[4] = 0x00; // version = 0
        var error = Assert.Throws<SnmpException>(() => SnmpMessage.Decode(bytes));
        Assert.Equal(SnmpError.UnsupportedVersion, error.Error);
        Assert.Equal(0, error.Version);
    }

    [Fact]
    public void Decode_UnknownPduType_ThrowsUnexpectedPduType()
    {
        var bytes = EncodedGet();
        bytes[13] = 0xA7;
        var error = Assert.Throws<SnmpException>(() => SnmpMessage.Decode(bytes));
        Assert.Equal(SnmpError.UnexpectedPduType, error.Error);
        Assert.Equal((byte)0xA7, error.PduTag);
    }

    [Fact]
    public void Decode_RequestIdOutsideInt32_ThrowsMalformedWithIntegerOverflow()
    {
        var get = EncodedGet();
        // Change request-id to 2^31 (02 05 00 80 00 00 00, 4 bytes longer); each of the two outer lengths grows by 4.
        byte[] bytes = [0x30, 0x2A, .. get[2..13], 0xA0, 0x1D, 0x02, 0x05, 0x00, 0x80, 0x00, 0x00, 0x00, .. get[18..]];
        Assert.Equal(BerError.IntegerOverflow, MalformedCause(bytes));
    }

    [Fact]
    public void Decode_TruncatedMessage_ThrowsMalformedWithTruncated()
    {
        Assert.Equal(BerError.Truncated, MalformedCause(EncodedGet()[..^3]));
    }

    [Fact]
    public void Decode_Garbage_ThrowsMalformed()
    {
        var error = Assert.Throws<SnmpException>(() => SnmpMessage.Decode([0xDE, 0xAD, 0xBE, 0xEF]));
        Assert.Equal(SnmpError.Malformed, error.Error);
    }

    [Fact]
    public void Decode_VarBindWithIncompleteValue_ThrowsMalformed()
    {
        byte[] bytes =
        [
            0x30, 0x1B,
            0x02, 0x01, 0x01,
            0x04, 0x06, 0x70, 0x75, 0x62, 0x6C, 0x69, 0x63,
            0xA2, 0x0E,
            0x02, 0x01, 0x01,
            0x02, 0x01, 0x00,
            0x02, 0x01, 0x00,
            0x30, 0x03,
            0x30, 0x01,                                         // The varbind has only one incomplete value
            0x05,
        ];
        Assert.Equal(BerError.Truncated, MalformedCause(bytes));
    }

    [Fact]
    public void Decode_CommunityNotOctetString_ThrowsMalformed()
    {
        var bytes = EncodedGet();
        bytes[5] = 0x02; // Change the community tag to INTEGER
        Assert.Equal(BerError.InvalidContent, MalformedCause(bytes));
    }

    private static byte[] EncodedGet() => new SnmpMessage("public", Pdu.Get(1, SnmpOids.SysUpTime)).Encode();

    private static BerError MalformedCause(byte[] bytes)
    {
        var error = Assert.Throws<SnmpException>(() => SnmpMessage.Decode(bytes));
        Assert.Equal(SnmpError.Malformed, error.Error);
        return Assert.IsType<BerException>(error.InnerException).Error;
    }
}
