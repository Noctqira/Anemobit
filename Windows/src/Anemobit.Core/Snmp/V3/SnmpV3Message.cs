using Anemobit.Core.Ber;

namespace Anemobit.Core.Snmp.V3;

/// <summary>SNMPv3 message (RFC 3412, USM security model).</summary>
internal sealed record SnmpV3Message(int MsgId, SnmpV3Flags Flags, UsmSecurityParameters Security, SnmpV3Data Data)
{
    public const long Version = 3;
    public const long UsmSecurityModel = 3;
    public const long MaxSize = 65_507;

    /// <summary>Decodes and returns where the msgAuthenticationParameters contents are (they must be zeroed to verify the HMAC).</summary>
    /// <exception cref="SnmpException">Malformed, version isn't 3, or the security model isn't USM.</exception>
    public static (SnmpV3Message Message, Range AuthParamsRange) Decode(byte[] bytes) => SnmpException.WrapMalformed(() =>
    {
        var message = new BerReader(bytes).ReadSequence();

        var version = message.ReadInteger();
        if (version != Version)
        {
            throw SnmpException.UnsupportedVersion(version);
        }

        var global = message.ReadSequence();
        var msgId = global.ReadInteger();
        global.ReadInteger(); // msgMaxSize
        var flagBytes = global.ReadOctetString();
        if (flagBytes.Length != 1)
        {
            throw new BerException(BerError.InvalidContent, BerTag.OctetString);
        }
        if (global.ReadInteger() != UsmSecurityModel)
        {
            throw SnmpException.UnexpectedResponse("unsupported security model");
        }

        var (securityTag, securityRange) = message.ReadTlvRange();
        if (securityTag != BerTag.OctetString)
        {
            throw new BerException(BerError.UnexpectedTag, securityTag, BerTag.OctetString);
        }
        var usm = message.SubReader(securityRange).ReadSequence();
        var engineId = usm.ReadOctetString();
        var engineBoots = ReadUInt32(usm);
        var engineTime = ReadUInt32(usm);
        var userName = usm.ReadOctetString();
        var (authTag, authRange) = usm.ReadTlvRange();
        if (authTag != BerTag.OctetString)
        {
            throw new BerException(BerError.UnexpectedTag, authTag, BerTag.OctetString);
        }
        var authParams = usm.BytesIn(authRange);
        var privParams = usm.ReadOctetString();

        SnmpV3Data data = message.PeekTag() == BerTag.OctetString
            ? new SnmpV3Data.Encrypted(message.ReadOctetString())
            : new SnmpV3Data.Plaintext(ScopedPdu.Decode(message));

        if (msgId is < int.MinValue or > int.MaxValue)
        {
            throw new BerException(BerError.IntegerOverflow, BerTag.Integer);
        }
        var security = new UsmSecurityParameters(engineId, engineBoots, engineTime, userName, authParams, privParams);
        return (new SnmpV3Message((int)msgId, (SnmpV3Flags)flagBytes[0], security, data), authRange);
    });

    /// <summary>Encoded bytes and the position of the authParams contents (null when authParams is empty), for filling in the HMAC.</summary>
    public (byte[] Bytes, Range? AuthParamsRange) Encode()
    {
        var globalData = BerEncoding.EncodeTlv(
            BerTag.Sequence,
            [
                .. new SnmpValue.Integer32(MsgId).Encode(),
                .. new SnmpValue.Integer32(MaxSize).Encode(),
                .. BerEncoding.EncodeTlv(BerTag.OctetString, [(byte)Flags]),
                .. new SnmpValue.Integer32(UsmSecurityModel).Encode(),
            ]);

        byte[] beforeAuth =
        [
            .. BerEncoding.EncodeTlv(BerTag.OctetString, Security.EngineId),
            .. new SnmpValue.Integer32(Security.EngineBoots).Encode(),
            .. new SnmpValue.Integer32(Security.EngineTime).Encode(),
            .. BerEncoding.EncodeTlv(BerTag.OctetString, Security.UserName),
        ];
        var authTlv = BerEncoding.EncodeTlv(BerTag.OctetString, Security.AuthParams);
        byte[] usmContent = [.. beforeAuth, .. authTlv, .. BerEncoding.EncodeTlv(BerTag.OctetString, Security.PrivParams)];
        var usmSequence = BerEncoding.EncodeTlv(BerTag.Sequence, usmContent);
        var securityParameters = BerEncoding.EncodeTlv(BerTag.OctetString, usmSequence);

        var dataTlv = Data switch
        {
            SnmpV3Data.Plaintext plain => plain.ScopedPdu.Encode(),
            SnmpV3Data.Encrypted encrypted => BerEncoding.EncodeTlv(BerTag.OctetString, encrypted.Ciphertext),
            _ => throw new InvalidOperationException("Unknown SNMPv3 data."),
        };

        var version = new SnmpValue.Integer32(Version).Encode();
        byte[] content = [.. version, .. globalData, .. securityParameters, .. dataTlv];
        var bytes = BerEncoding.EncodeTlv(BerTag.Sequence, content);

        if (Security.AuthParams.Length == 0)
        {
            return (bytes, null);
        }
        // Add up the TLV header lengths from the outside in to find where the authParams contents are.
        var outerHeader = bytes.Length - content.Length;
        var securityHeader = securityParameters.Length - usmSequence.Length;
        var usmHeader = usmSequence.Length - usmContent.Length;
        var authHeader = authTlv.Length - Security.AuthParams.Length;
        var start = outerHeader + version.Length + globalData.Length + securityHeader + usmHeader + beforeAuth.Length + authHeader;
        return (bytes, start..(start + Security.AuthParams.Length));
    }

    private static uint ReadUInt32(BerReader reader)
    {
        var value = reader.ReadInteger();
        return value is >= 0 and <= uint.MaxValue
            ? (uint)value
            : throw new BerException(BerError.IntegerOverflow, BerTag.Integer);
    }
}
