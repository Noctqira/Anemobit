using System.Buffers.Binary;
using Anemobit.Core.Snmp;

namespace Anemobit.Core.Ber;

/// <summary>Low-level helpers for BER encoding and decoding primitive contents.</summary>
internal static class BerEncoding
{
    public static byte[] EncodeLength(int length)
    {
        ArgumentOutOfRangeException.ThrowIfNegative(length);
        if (length < 0x80)
        {
            return [(byte)length];
        }

        var bytes = new List<byte>();
        for (var value = length; value > 0; value >>= 8)
        {
            bytes.Insert(0, (byte)(value & 0xFF));
        }
        return [(byte)(0x80 | bytes.Count), .. bytes];
    }

    public static byte[] EncodeTlv(byte tag, ReadOnlySpan<byte> content) =>
        [tag, .. EncodeLength(content.Length), .. content];

    /// <summary>Shortest two's-complement representation.</summary>
    public static byte[] EncodeSigned(long value)
    {
        Span<byte> bytes = stackalloc byte[8];
        BinaryPrimitives.WriteInt64BigEndian(bytes, value);
        var start = 0;
        while (start < 7)
        {
            byte first = bytes[start], second = bytes[start + 1];
            var redundant = (first == 0x00 && (second & 0x80) == 0) || (first == 0xFF && (second & 0x80) != 0);
            if (!redundant)
            {
                break;
            }
            start++;
        }
        return bytes[start..].ToArray();
    }

    /// <summary>Unsigned integer: strip leading zeros, and prepend 0x00 when the high bit is 1 so it isn't read as negative.</summary>
    public static byte[] EncodeUnsigned(ulong value)
    {
        Span<byte> bytes = stackalloc byte[8];
        BinaryPrimitives.WriteUInt64BigEndian(bytes, value);
        var start = 0;
        while (start < 7 && bytes[start] == 0)
        {
            start++;
        }
        var digits = bytes[start..];
        return (digits[0] & 0x80) != 0 ? [0x00, .. digits] : digits.ToArray();
    }

    public static byte[] EncodeOidContent(Oid oid)
    {
        var arcs = oid.Arcs;
        ulong first = arcs.Length > 0 ? arcs[0] : 0;
        ulong second = arcs.Length > 1 ? arcs[1] : 0;
        var bytes = new List<byte>();
        AppendBase128(bytes, (first * 40) + second);
        foreach (var arc in arcs.AsSpan()[Math.Min(2, arcs.Length)..])
        {
            AppendBase128(bytes, arc);
        }
        return [.. bytes];
    }

    public static long DecodeSigned(ReadOnlySpan<byte> content, byte tag)
    {
        if (content.IsEmpty)
        {
            throw new BerException(BerError.InvalidContent, tag);
        }
        if (content.Length > 8)
        {
            throw new BerException(BerError.IntegerOverflow, tag);
        }

        long value = (content[0] & 0x80) != 0 ? -1 : 0;
        foreach (var b in content)
        {
            value = (value << 8) | b;
        }
        return value;
    }

    public static ulong DecodeUnsigned(ReadOnlySpan<byte> content, byte tag, int bitWidth)
    {
        if (content.IsEmpty)
        {
            throw new BerException(BerError.InvalidContent, tag);
        }

        var digits = content;
        while (digits.Length > 1 && digits[0] == 0)
        {
            digits = digits[1..];
        }
        if (digits.Length > bitWidth / 8)
        {
            throw new BerException(BerError.IntegerOverflow, tag);
        }

        ulong value = 0;
        foreach (var b in digits)
        {
            value = (value << 8) | b;
        }
        return value;
    }

    public static Oid DecodeOidContent(ReadOnlySpan<byte> content)
    {
        if (content.IsEmpty)
        {
            throw new BerException(BerError.InvalidOid, BerTag.Oid);
        }

        var subidentifiers = new List<ulong>();
        ulong current = 0;
        var inProgress = false;
        foreach (var b in content)
        {
            // Guard against ulong overflow first; whether an arc exceeds uint is checked after splitting.
            if (current >> 57 != 0)
            {
                throw new BerException(BerError.InvalidOid, BerTag.Oid);
            }
            current = (current << 7) | ((ulong)b & 0x7F);
            inProgress = (b & 0x80) != 0;
            if (!inProgress)
            {
                subidentifiers.Add(current);
                current = 0;
            }
        }
        if (inProgress)
        {
            throw new BerException(BerError.InvalidOid, BerTag.Oid);
        }

        var combined = subidentifiers[0];
        var first = combined < 40 ? 0UL : combined < 80 ? 1UL : 2UL;
        var arcs = new uint[subidentifiers.Count + 1];
        arcs[0] = (uint)first;
        for (var i = 0; i < subidentifiers.Count; i++)
        {
            var sub = i == 0 ? combined - (first * 40) : subidentifiers[i];
            if (sub > uint.MaxValue)
            {
                throw new BerException(BerError.InvalidOid, BerTag.Oid);
            }
            arcs[i + 1] = (uint)sub;
        }
        return new Oid(arcs);
    }

    private static void AppendBase128(List<byte> output, ulong value)
    {
        var insertAt = output.Count;
        output.Add((byte)(value & 0x7F));
        for (var rest = value >> 7; rest > 0; rest >>= 7)
        {
            output.Insert(insertAt, (byte)((rest & 0x7F) | 0x80));
        }
    }
}
