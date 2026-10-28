namespace Anemobit.Core.Snmp;

/// <summary>Error kinds for <see cref="SnmpException"/> (similar to <c>HttpRequestException.HttpRequestError</c>).</summary>
public enum SnmpError
{
    /// <summary>Malformed packet; details in <see cref="Exception.InnerException"/>.</summary>
    Malformed,

    UnsupportedVersion,

    UnexpectedPduType,

    /// <summary>The response has a non-zero error-status; see <see cref="SnmpException.Status"/> / <see cref="SnmpException.ErrorIndex"/>.</summary>
    ErrorStatus,

    /// <summary>All retries timed out.</summary>
    Timeout,

    /// <summary>The response's varbinds don't match the request, or a required value is missing.</summary>
    UnexpectedResponse,

    /// <summary>No interface with the given name; see <see cref="SnmpException.InterfaceName"/>.</summary>
    InterfaceNotFound,

    /// <summary>Transport error (connection failed, socket closed, etc.).</summary>
    Transport,

    /// <summary>SNMPv3 authentication / privacy failure; see <see cref="SnmpException.UsmFailure"/> for why.</summary>
    Authentication,
}
