using System.Collections.Immutable;
using System.Text;
using Anemobit.Core.Ber;

namespace Anemobit.Core.Snmp;

/// <summary>Values that can appear in an SNMP varbind. Each type is a nested sealed record; use pattern matching to get values.</summary>
public abstract record SnmpValue
{
    private SnmpValue()
    {
    }

    /// <summary>noSuchObject / noSuchInstance / endOfMibView.</summary>
    public bool IsException => this is NoSuchObject or NoSuchInstance or EndOfMibView;

    /// <summary>Numeric types as ulong (counter / gauge / timeTicks / non-negative integer); null for other types.</summary>
    public ulong? UnsignedValue => this switch
    {
        Counter64 v => v.Value,
        Counter32 v => v.Value,
        Gauge32 v => v.Value,
        TimeTicks v => v.Value,
        Integer32 { Value: >= 0 } v => (ulong)v.Value,
        _ => null,
    };

    /// <summary>OCTET STRING decoded as UTF-8 (for ifName / ifDescr); null for other types.</summary>
    public string? StringValue => this is OctetString s ? Encoding.UTF8.GetString(s.Value.AsSpan()) : null;

    internal static SnmpValue Decode(BerReader reader)
    {
        var (tag, memory) = reader.ReadTlv();
        var content = memory.Span;
        return tag switch
        {
            BerTag.Integer => new Integer32(BerEncoding.DecodeSigned(content, tag)),
            BerTag.OctetString => new OctetString([.. content]),
            BerTag.Null => content.IsEmpty ? new Null() : throw new BerException(BerError.InvalidContent, tag),
            BerTag.Oid => new ObjectId(BerEncoding.DecodeOidContent(content)),
            BerTag.IpAddress => content.Length == 4
                ? new IpAddress([.. content])
                : throw new BerException(BerError.InvalidContent, tag),
            BerTag.Counter32 => new Counter32((uint)BerEncoding.DecodeUnsigned(content, tag, 32)),
            BerTag.Gauge32 => new Gauge32((uint)BerEncoding.DecodeUnsigned(content, tag, 32)),
            BerTag.TimeTicks => new TimeTicks((uint)BerEncoding.DecodeUnsigned(content, tag, 32)),
            BerTag.Counter64 => new Counter64(BerEncoding.DecodeUnsigned(content, tag, 64)),
            BerTag.NoSuchObject => new NoSuchObject(),
            BerTag.NoSuchInstance => new NoSuchInstance(),
            BerTag.EndOfMibView => new EndOfMibView(),
            _ => throw new BerException(BerError.UnknownTag, tag),
        };
    }

    internal abstract byte[] Encode();

    private static int HashBytes(ImmutableArray<byte> bytes)
    {
        var hash = new HashCode();
        hash.AddBytes(bytes.AsSpan());
        return hash.ToHashCode();
    }

    public sealed record Integer32(long Value) : SnmpValue
    {
        internal override byte[] Encode() => BerEncoding.EncodeTlv(BerTag.Integer, BerEncoding.EncodeSigned(Value));
    }

    public sealed record OctetString(ImmutableArray<byte> Value) : SnmpValue
    {
        public bool Equals(OctetString? other) => other is not null && Value.AsSpan().SequenceEqual(other.Value.AsSpan());

        public override int GetHashCode() => HashBytes(Value);

        public override string ToString() => $"OctetString {{ {Convert.ToHexString(Value.AsSpan())} }}";

        internal override byte[] Encode() => BerEncoding.EncodeTlv(BerTag.OctetString, Value.AsSpan());
    }

    public sealed record Null : SnmpValue
    {
        internal override byte[] Encode() => BerEncoding.EncodeTlv(BerTag.Null, []);
    }

    public sealed record ObjectId(Oid Value) : SnmpValue
    {
        internal override byte[] Encode() => BerEncoding.EncodeTlv(BerTag.Oid, BerEncoding.EncodeOidContent(Value));
    }

    public sealed record IpAddress(ImmutableArray<byte> Value) : SnmpValue
    {
        public bool Equals(IpAddress? other) => other is not null && Value.AsSpan().SequenceEqual(other.Value.AsSpan());

        public override int GetHashCode() => HashBytes(Value);

        public override string ToString() => $"IpAddress {{ {string.Join('.', Value)} }}";

        internal override byte[] Encode() => BerEncoding.EncodeTlv(BerTag.IpAddress, Value.AsSpan());
    }

    public sealed record Counter32(uint Value) : SnmpValue
    {
        internal override byte[] Encode() => BerEncoding.EncodeTlv(BerTag.Counter32, BerEncoding.EncodeUnsigned(Value));
    }

    public sealed record Gauge32(uint Value) : SnmpValue
    {
        internal override byte[] Encode() => BerEncoding.EncodeTlv(BerTag.Gauge32, BerEncoding.EncodeUnsigned(Value));
    }

    public sealed record TimeTicks(uint Value) : SnmpValue
    {
        internal override byte[] Encode() => BerEncoding.EncodeTlv(BerTag.TimeTicks, BerEncoding.EncodeUnsigned(Value));
    }

    public sealed record Counter64(ulong Value) : SnmpValue
    {
        internal override byte[] Encode() => BerEncoding.EncodeTlv(BerTag.Counter64, BerEncoding.EncodeUnsigned(Value));
    }

    public sealed record NoSuchObject : SnmpValue
    {
        internal override byte[] Encode() => BerEncoding.EncodeTlv(BerTag.NoSuchObject, []);
    }

    public sealed record NoSuchInstance : SnmpValue
    {
        internal override byte[] Encode() => BerEncoding.EncodeTlv(BerTag.NoSuchInstance, []);
    }

    public sealed record EndOfMibView : SnmpValue
    {
        internal override byte[] Encode() => BerEncoding.EncodeTlv(BerTag.EndOfMibView, []);
    }
}
