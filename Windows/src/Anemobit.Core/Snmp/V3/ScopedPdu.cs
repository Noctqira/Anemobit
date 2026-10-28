using Anemobit.Core.Ber;

namespace Anemobit.Core.Snmp.V3;

/// <summary>ScopedPDU (RFC 3412): contextEngineID, contextName and PDU.</summary>
internal sealed record ScopedPdu(byte[] ContextEngineId, byte[] ContextName, Pdu Pdu)
{
    public static ScopedPdu Decode(BerReader reader)
    {
        var scoped = reader.ReadSequence();
        var engineId = scoped.ReadOctetString();
        var contextName = scoped.ReadOctetString();
        return new ScopedPdu(engineId, contextName, Pdu.Decode(scoped));
    }

    public byte[] Encode() => BerEncoding.EncodeTlv(
        BerTag.Sequence,
        [
            .. BerEncoding.EncodeTlv(BerTag.OctetString, ContextEngineId),
            .. BerEncoding.EncodeTlv(BerTag.OctetString, ContextName),
            .. Pdu.Encode(),
        ]);

    public bool Equals(ScopedPdu? other) =>
        other is not null
        && ContextEngineId.AsSpan().SequenceEqual(other.ContextEngineId)
        && ContextName.AsSpan().SequenceEqual(other.ContextName)
        && Pdu.Equals(other.Pdu);

    public override int GetHashCode() => HashCode.Combine(ContextEngineId.Length, ContextName.Length, Pdu);
}
