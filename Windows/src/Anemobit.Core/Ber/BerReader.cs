namespace Anemobit.Core.Ber;

/// <summary>Cursor that reads BER TLVs sequentially.</summary>
internal sealed class BerReader
{
    private readonly byte[] _bytes;
    private readonly int _end;
    private int _offset;

    public BerReader(byte[] bytes)
        : this(bytes, 0, bytes.Length)
    {
    }

    private BerReader(byte[] bytes, int start, int end)
    {
        _bytes = bytes;
        _offset = start;
        _end = end;
    }

    public bool IsAtEnd => _offset >= _end;

    /// <summary>Peeks at the next tag without moving the cursor.</summary>
    public byte PeekTag() =>
        _offset < _end ? _bytes[_offset] : throw new BerException(BerError.Truncated);

    /// <summary>Reads one TLV and returns the tag and contents.</summary>
    public (byte Tag, ReadOnlyMemory<byte> Content) ReadTlv()
    {
        var (tag, range) = ReadHeader();
        return (tag, _bytes.AsMemory(range));
    }

    /// <summary>Reads one TLV and returns where its contents are in the original bytes (needed to verify the SNMPv3 HMAC).</summary>
    public (byte Tag, Range Range) ReadTlvRange() => ReadHeader();

    /// <summary>Reads a constructed value with the given tag and returns a child reader that sees only its contents.</summary>
    public BerReader ReadConstructed(byte expectedTag)
    {
        var (tag, range) = ReadHeader();
        if (tag != expectedTag)
        {
            throw new BerException(BerError.UnexpectedTag, tag, expectedTag);
        }
        return SubReader(range);
    }

    public BerReader ReadSequence() => ReadConstructed(BerTag.Sequence);

    /// <summary>Creates a child reader over a range of the original bytes (e.g. BER inside an OCTET STRING).</summary>
    public BerReader SubReader(Range range)
    {
        var (start, length) = range.GetOffsetAndLength(_bytes.Length);
        return new BerReader(_bytes, start, start + length);
    }

    /// <summary>Returns a range of the original bytes.</summary>
    public byte[] BytesIn(Range range) => _bytes[range];

    private (byte Tag, Range Range) ReadHeader()
    {
        if (_offset >= _end)
        {
            throw new BerException(BerError.Truncated);
        }
        var tag = _bytes[_offset++];
        if (_offset >= _end)
        {
            throw new BerException(BerError.Truncated, tag);
        }
        var first = _bytes[_offset++];

        long length;
        if ((first & 0x80) == 0)
        {
            length = first;
        }
        else
        {
            var count = first & 0x7F;
            if (count == 0)
            {
                throw new BerException(BerError.IndefiniteLength, tag);
            }
            if (count > 4)
            {
                throw new BerException(BerError.LengthTooLarge, tag);
            }
            if (_end - _offset < count)
            {
                throw new BerException(BerError.Truncated, tag);
            }

            length = 0;
            for (var i = 0; i < count; i++)
            {
                length = (length << 8) | _bytes[_offset++];
            }
        }

        if (_end - _offset < length)
        {
            throw new BerException(BerError.Truncated, tag);
        }
        var start = _offset;
        _offset += (int)length;
        return (tag, start.._offset);
    }
}
