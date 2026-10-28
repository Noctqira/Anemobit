using Anemobit.Core.Snmp;
using Anemobit.Core.Snmp.V3;

namespace Anemobit.Core.Tests.Snmp.V3;

public class SnmpV3MessageTests
{
    [Fact]
    public void Encode_DiscoveryRequest_MatchesReferenceBytes()
    {
        var message = new SnmpV3Message(
            1,
            SnmpV3Flags.Reportable,
            new UsmSecurityParameters(),
            new SnmpV3Data.Plaintext(new ScopedPdu([], [], Pdu.Get(1))));
        byte[] expected =
        [
            0x30, 0x38,
            0x02, 0x01, 0x03,                               // msgVersion 3
            0x30, 0x0E,                                     // msgGlobalData
            0x02, 0x01, 0x01,                               //   msgID
            0x02, 0x03, 0x00, 0xFF, 0xE3,                   //   msgMaxSize 65507
            0x04, 0x01, 0x04,                               //   msgFlags reportable
            0x02, 0x01, 0x03,                               //   msgSecurityModel USM
            0x04, 0x10,                                     // msgSecurityParameters
            0x30, 0x0E,
            0x04, 0x00,                                     //   engineID
            0x02, 0x01, 0x00,                               //   engineBoots
            0x02, 0x01, 0x00,                               //   engineTime
            0x04, 0x00,                                     //   userName
            0x04, 0x00,                                     //   authParams
            0x04, 0x00,                                     //   privParams
            0x30, 0x11,                                     // ScopedPDU
            0x04, 0x00,                                     //   contextEngineID
            0x04, 0x00,                                     //   contextName
            0xA0, 0x0B, 0x02, 0x01, 0x01, 0x02, 0x01, 0x00, 0x02, 0x01, 0x00, 0x30, 0x00,
        ];

        var (bytes, authRange) = message.Encode();

        Assert.Equal(expected, bytes);
        Assert.Null(authRange);
    }

    [Fact]
    public void Decode_PlaintextWithAuthParams_RoundTripsAndLocatesAuthParams()
    {
        var security = new UsmSecurityParameters(
            EngineId: [0x80, 0x00, 0x3A, 0x8C, 0x04, 0x01],
            EngineBoots: 12,
            EngineTime: 345_678,
            UserName: "anemobit"u8.ToArray(),
            AuthParams: new byte[12],
            PrivParams: []);
        var message = new SnmpV3Message(
            42,
            SnmpV3Flags.Reportable | SnmpV3Flags.Auth,
            security,
            new SnmpV3Data.Plaintext(new ScopedPdu(security.EngineId, [], Pdu.Get(42, SnmpOids.SysUpTime))));

        var (bytes, authRange) = message.Encode();

        Assert.NotNull(authRange);
        var (offset, length) = authRange.Value.GetOffsetAndLength(bytes.Length);
        Assert.Equal(12, length);
        Assert.All(bytes[authRange.Value], b => Assert.Equal(0, b));

        var (decoded, decodedRange) = SnmpV3Message.Decode(bytes);
        Assert.Equal(message, decoded);
        Assert.Equal((offset, length), decodedRange.GetOffsetAndLength(bytes.Length));
    }

    [Fact]
    public void Decode_EncryptedData_RoundTrips()
    {
        var message = new SnmpV3Message(
            7,
            SnmpV3Flags.Auth | SnmpV3Flags.Priv,
            new UsmSecurityParameters([1, 2, 3], 1, 2, [0x61], new byte[12], [1, 2, 3, 4, 5, 6, 7, 8]),
            new SnmpV3Data.Encrypted([0xDE, 0xAD, 0xBE, 0xEF]));

        Assert.Equal(message, SnmpV3Message.Decode(message.Encode().Bytes).Message);
    }

    [Fact]
    public void Decode_MaximumEngineBootsAndTime_RoundTrips()
    {
        // engineBoots / engineTime range is 0…2147483647, encoded as INTEGER
        var message = new SnmpV3Message(
            1,
            SnmpV3Flags.None,
            new UsmSecurityParameters(EngineBoots: 2_147_483_647, EngineTime: 2_147_483_647),
            new SnmpV3Data.Encrypted([]));

        Assert.Equal(message, SnmpV3Message.Decode(message.Encode().Bytes).Message);
    }

    [Fact]
    public void Decode_V2cMessage_ThrowsUnsupportedVersion()
    {
        var v2c = new SnmpMessage("public", Pdu.Get(1, SnmpOids.SysUpTime)).Encode();
        var error = Assert.Throws<SnmpException>(() => SnmpV3Message.Decode(v2c));
        Assert.Equal(SnmpError.UnsupportedVersion, error.Error);
        Assert.Equal(1, error.Version);
    }

    [Fact]
    public void Decode_NonUsmSecurityModel_ThrowsUnexpectedResponse()
    {
        var (bytes, _) = new SnmpV3Message(
            1,
            SnmpV3Flags.Reportable,
            new UsmSecurityParameters(),
            new SnmpV3Data.Plaintext(new ScopedPdu([], [], Pdu.Get(1)))).Encode();
        bytes[20] = 0x02; // msgSecurityModel 3 → 2

        var error = Assert.Throws<SnmpException>(() => SnmpV3Message.Decode(bytes));
        Assert.Equal(SnmpError.UnexpectedResponse, error.Error);
    }

    [Fact]
    public void ToString_SecurityParameters_DoesNotExposeKeysOrUserName()
    {
        var security = new UsmSecurityParameters([1], 1, 1, "secret-user"u8.ToArray(), [0xAB], [0xCD]);
        Assert.DoesNotContain("secret", security.ToString(), StringComparison.Ordinal);
    }
}
