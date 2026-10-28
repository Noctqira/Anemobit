namespace Anemobit.Core.Ber;

/// <summary>
/// BER decoding failed. <see cref="Tag"/> is the offending tag; for <see cref="BerError.UnexpectedTag"/> there's also <see cref="ExpectedTag"/>.
/// Always wrapped as <see cref="Snmp.SnmpException"/> (<see cref="Snmp.SnmpError.Malformed"/>) for callers.
/// </summary>
internal sealed class BerException : Exception
{
    public BerException(BerError error, byte? tag = null, byte? expectedTag = null)
        : base(Describe(error, tag, expectedTag))
    {
        Error = error;
        Tag = tag;
        ExpectedTag = expectedTag;
    }

    public BerError Error { get; }

    public byte? Tag { get; }

    public byte? ExpectedTag { get; }

    private static string Describe(BerError error, byte? tag, byte? expectedTag) => (tag, expectedTag) switch
    {
        ({ } actual, { } expected) => $"BER {error}: expected tag 0x{expected:X2}, got 0x{actual:X2}",
        ({ } actual, null) => $"BER {error}: tag 0x{actual:X2}",
        _ => $"BER {error}",
    };
}
