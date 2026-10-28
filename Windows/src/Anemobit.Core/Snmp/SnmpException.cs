using System.Globalization;
using Anemobit.Core.Ber;

namespace Anemobit.Core.Snmp;

/// <summary>
/// SNMP-layer error; see <see cref="Error"/> for the kind. <see cref="Exception.Message"/> is English for logs;
/// the app localizes UI text from <see cref="Error"/>.
/// </summary>
public sealed class SnmpException : Exception
{
    private SnmpException(SnmpError error, string message, Exception? innerException = null)
        : base(message, innerException)
    {
        Error = error;
    }

    public SnmpError Error { get; }

    /// <summary>The version received, for <see cref="SnmpError.UnsupportedVersion"/>.</summary>
    public long? Version { get; private init; }

    /// <summary>The tag received, for <see cref="SnmpError.UnexpectedPduType"/>.</summary>
    public byte? PduTag { get; private init; }

    /// <summary>The error-status value, for <see cref="SnmpError.ErrorStatus"/>.</summary>
    public long? Status { get; private init; }

    /// <summary>The 1-based varbind position, for <see cref="SnmpError.ErrorStatus"/> (0 means the whole request).</summary>
    public long? ErrorIndex { get; private init; }

    /// <summary>The interface name that wasn't found, for <see cref="SnmpError.InterfaceNotFound"/>.</summary>
    public string? InterfaceName { get; private init; }

    /// <summary>Why it failed, for <see cref="SnmpError.Authentication"/>.</summary>
    public UsmFailure? UsmFailure { get; private init; }

    internal static SnmpException Malformed(BerException inner) =>
        new(SnmpError.Malformed, $"Malformed packet ({inner.Message})", inner);

    internal static SnmpException UnsupportedVersion(long version) =>
        new(SnmpError.UnsupportedVersion, $"Unsupported SNMP version {version}") { Version = version };

    internal static SnmpException UnexpectedPduType(byte tag) =>
        new(SnmpError.UnexpectedPduType, $"Unexpected PDU type 0x{tag:X2}") { PduTag = tag };

    internal static SnmpException FromErrorStatus(long status, long index)
    {
        var name = status is >= int.MinValue and <= int.MaxValue && Enum.IsDefined((SnmpErrorStatus)status)
            ? ((SnmpErrorStatus)status).ToString()
            : status.ToString(CultureInfo.InvariantCulture);
        return new(SnmpError.ErrorStatus, $"SNMP error {name} (varbind {index})") { Status = status, ErrorIndex = index };
    }

    internal static SnmpException Timeout() => new(SnmpError.Timeout, "Connection timed out");

    internal static SnmpException UnexpectedResponse(string detail) =>
        new(SnmpError.UnexpectedResponse, $"Unexpected response: {detail}");

    internal static SnmpException InterfaceNotFound(string name) =>
        new(SnmpError.InterfaceNotFound, $"Interface {name} not found") { InterfaceName = name };

    internal static SnmpException Transport(string detail, Exception? inner = null) =>
        new(SnmpError.Transport, $"Connection error: {detail}", inner);

    internal static SnmpException Authentication(UsmFailure failure, Exception? inner = null) =>
        new(SnmpError.Authentication, $"SNMPv3 authentication failed: {failure}", inner) { UsmFailure = failure };

    /// <summary>Turns any <see cref="BerException"/> during decoding into <see cref="SnmpError.Malformed"/>.</summary>
    internal static T WrapMalformed<T>(Func<T> decode)
    {
        try
        {
            return decode();
        }
        catch (BerException error)
        {
            throw Malformed(error);
        }
    }
}
