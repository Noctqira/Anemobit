using Anemobit.Core.Ber;

namespace Anemobit.Core.Snmp;

/// <summary>SNMP PDU. For GetBulk, <see cref="ErrorStatus"/> / <see cref="ErrorIndex"/> carry non-repeaters / max-repetitions.</summary>
internal sealed record Pdu(PduType Type, int RequestId, long ErrorStatus, long ErrorIndex, IReadOnlyList<VarBind> VarBinds)
{
    public SnmpErrorStatus? KnownErrorStatus =>
        ErrorStatus is >= int.MinValue and <= int.MaxValue && Enum.IsDefined((SnmpErrorStatus)ErrorStatus)
            ? (SnmpErrorStatus)ErrorStatus
            : null;

    public static Pdu Get(int requestId, params IEnumerable<Oid> oids) =>
        new(PduType.Get, requestId, 0, 0, NullBinds(oids));

    public static Pdu GetNext(int requestId, params IEnumerable<Oid> oids) =>
        new(PduType.GetNext, requestId, 0, 0, NullBinds(oids));

    public static Pdu GetBulk(int requestId, int nonRepeaters, int maxRepetitions, params IEnumerable<Oid> oids) =>
        new(PduType.GetBulk, requestId, nonRepeaters, maxRepetitions, NullBinds(oids));

    public static Pdu Decode(BerReader reader)
    {
        var rawType = reader.PeekTag();
        if (!Enum.IsDefined((PduType)rawType))
        {
            throw SnmpException.UnexpectedPduType(rawType);
        }
        var pduReader = reader.ReadConstructed(rawType);

        var requestId = pduReader.ReadInteger();
        if (requestId is < int.MinValue or > int.MaxValue)
        {
            throw new BerException(BerError.IntegerOverflow, BerTag.Integer);
        }
        var errorStatus = pduReader.ReadInteger();
        var errorIndex = pduReader.ReadInteger();

        var list = pduReader.ReadSequence();
        var varBinds = new List<VarBind>();
        while (!list.IsAtEnd)
        {
            var bind = list.ReadSequence();
            if (SnmpValue.Decode(bind) is not SnmpValue.ObjectId oid)
            {
                throw new BerException(BerError.InvalidContent, BerTag.Oid);
            }
            varBinds.Add(new VarBind(oid.Value, SnmpValue.Decode(bind)));
        }
        return new Pdu((PduType)rawType, (int)requestId, errorStatus, errorIndex, varBinds);
    }

    public byte[] Encode()
    {
        var varBindList = new List<byte>();
        foreach (var bind in VarBinds)
        {
            varBindList.AddRange(BerEncoding.EncodeTlv(
                BerTag.Sequence,
                [.. new SnmpValue.ObjectId(bind.Oid).Encode(), .. bind.Value.Encode()]));
        }
        byte[] content =
        [
            .. new SnmpValue.Integer32(RequestId).Encode(),
            .. new SnmpValue.Integer32(ErrorStatus).Encode(),
            .. new SnmpValue.Integer32(ErrorIndex).Encode(),
            .. BerEncoding.EncodeTlv(BerTag.Sequence, [.. varBindList]),
        ];
        return BerEncoding.EncodeTlv((byte)Type, content);
    }

    public bool Equals(Pdu? other) =>
        other is not null
        && Type == other.Type
        && RequestId == other.RequestId
        && ErrorStatus == other.ErrorStatus
        && ErrorIndex == other.ErrorIndex
        && VarBinds.SequenceEqual(other.VarBinds);

    public override int GetHashCode()
    {
        var hash = new HashCode();
        hash.Add(Type);
        hash.Add(RequestId);
        hash.Add(ErrorStatus);
        hash.Add(ErrorIndex);
        foreach (var bind in VarBinds)
        {
            hash.Add(bind);
        }
        return hash.ToHashCode();
    }

    private static VarBind[] NullBinds(IEnumerable<Oid> oids) =>
        [.. oids.Select(oid => new VarBind(oid, new SnmpValue.Null()))];
}
