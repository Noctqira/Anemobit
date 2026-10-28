using System.Text;
using Anemobit.Core.Ber;

namespace Anemobit.Core.Snmp;

/// <summary>SNMPv2c message (version = 1).</summary>
internal sealed record SnmpMessage(byte[] Community, Pdu Pdu, long Version = SnmpMessage.VersionV2c)
{
    public const long VersionV2c = 1;

    public SnmpMessage(string community, Pdu pdu)
        : this(Encoding.UTF8.GetBytes(community), pdu)
    {
    }

    /// <summary>Decodes a v2c message; throws <see cref="SnmpException"/> when malformed.</summary>
    public static SnmpMessage Decode(byte[] bytes) => SnmpException.WrapMalformed(() =>
    {
        var message = new BerReader(bytes).ReadSequence();

        var version = message.ReadInteger();
        if (version != VersionV2c)
        {
            throw SnmpException.UnsupportedVersion(version);
        }
        var community = message.ReadOctetString();
        return new SnmpMessage(community, Pdu.Decode(message), version);
    });

    public byte[] Encode()
    {
        byte[] content =
        [
            .. new SnmpValue.Integer32(Version).Encode(),
            .. BerEncoding.EncodeTlv(BerTag.OctetString, Community),
            .. Pdu.Encode(),
        ];
        return BerEncoding.EncodeTlv(BerTag.Sequence, content);
    }

    public bool Equals(SnmpMessage? other) =>
        other is not null
        && Version == other.Version
        && Community.AsSpan().SequenceEqual(other.Community)
        && Pdu.Equals(other.Pdu);

    public override int GetHashCode()
    {
        var hash = new HashCode();
        hash.Add(Version);
        hash.AddBytes(Community);
        hash.Add(Pdu);
        return hash.ToHashCode();
    }
}
