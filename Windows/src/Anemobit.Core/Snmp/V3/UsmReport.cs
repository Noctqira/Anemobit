namespace Anemobit.Core.Snmp.V3;

/// <summary>OIDs of usmStats Reports (RFC 3414 §5).</summary>
internal static class UsmReport
{
    public static readonly Oid UnsupportedSecLevels = new(1, 3, 6, 1, 6, 3, 15, 1, 1, 1, 0);
    public static readonly Oid NotInTimeWindows = new(1, 3, 6, 1, 6, 3, 15, 1, 1, 2, 0);
    public static readonly Oid UnknownUserNames = new(1, 3, 6, 1, 6, 3, 15, 1, 1, 3, 0);
    public static readonly Oid UnknownEngineIds = new(1, 3, 6, 1, 6, 3, 15, 1, 1, 4, 0);
    public static readonly Oid WrongDigests = new(1, 3, 6, 1, 6, 3, 15, 1, 1, 5, 0);
    public static readonly Oid DecryptionErrors = new(1, 3, 6, 1, 6, 3, 15, 1, 1, 6, 0);

    public static SnmpException ErrorFor(Oid? oid)
    {
        UsmFailure? failure =
            oid == UnsupportedSecLevels ? UsmFailure.UnsupportedSecurityLevel
            : oid == NotInTimeWindows ? UsmFailure.NotInTimeWindow
            : oid == UnknownUserNames ? UsmFailure.UnknownUserName
            : oid == WrongDigests ? UsmFailure.WrongDigest
            : oid == DecryptionErrors ? UsmFailure.DecryptionError
            : oid == UnknownEngineIds ? UsmFailure.UnknownEngineId
            : null;
        return failure is { } known
            ? SnmpException.Authentication(known)
            : SnmpException.UnexpectedResponse($"report {oid?.ToString() ?? "-"}");
    }
}
