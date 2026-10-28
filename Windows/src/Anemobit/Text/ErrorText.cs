using System.Globalization;
using Anemobit.Core.Snmp;
using Anemobit.Resources;

namespace Anemobit.Text;

/// <summary>Turns Core errors into UI text (in the UI language). Core exception messages are for logs only.</summary>
public static class ErrorText
{
    public static string Describe(Exception error)
    {
        ArgumentNullException.ThrowIfNull(error);
        var culture = CultureInfo.CurrentCulture;
        return error switch
        {
            SnmpException snmp => Describe(snmp, culture),
            _ => string.Format(culture, Strings.ErrorConnectionFormat, error.Message),
        };
    }

    private static string Describe(SnmpException error, CultureInfo culture) => error.Error switch
    {
        SnmpError.Malformed => string.Format(culture, Strings.ErrorMalformedFormat, error.InnerException?.Message ?? error.Message),
        SnmpError.UnsupportedVersion => string.Format(culture, Strings.ErrorUnsupportedVersionFormat, error.Version),
        SnmpError.UnexpectedPduType => string.Format(culture, Strings.ErrorUnexpectedPduFormat, error.PduTag?.ToString("X2", CultureInfo.InvariantCulture)),
        SnmpError.ErrorStatus => string.Format(culture, Strings.ErrorStatusFormat, StatusName(error.Status), error.ErrorIndex),
        SnmpError.Timeout => Strings.ErrorTimeout,
        SnmpError.UnexpectedResponse => string.Format(culture, Strings.ErrorUnexpectedResponseFormat, Detail(error.Message)),
        SnmpError.InterfaceNotFound => string.Format(culture, Strings.ErrorInterfaceNotFoundFormat, error.InterfaceName),
        SnmpError.Transport => string.Format(culture, Strings.ErrorConnectionFormat, error.InnerException?.Message ?? Detail(error.Message)),
        SnmpError.Authentication => Describe(error.UsmFailure, culture),
        _ => error.Message,
    };

    private static string Describe(UsmFailure? failure, CultureInfo culture) => failure switch
    {
        UsmFailure.UnknownUserName => Strings.ErrorUnknownUser,
        UsmFailure.WrongDigest => Strings.ErrorWrongAuth,
        UsmFailure.DecryptionError => Strings.ErrorWrongPriv,
        UsmFailure.UnsupportedSecurityLevel => Strings.ErrorSecurityLevel,
        UsmFailure.NotInTimeWindow => Strings.ErrorTimeSync,
        UsmFailure.UnknownEngineId => Strings.ErrorEngineIdMismatch,
        UsmFailure.NoResponseToEncryptedRequest => Strings.ErrorNoResponseToEncrypted,
        UsmFailure.DiscoveryIncomplete => Strings.ErrorDiscoveryIncomplete,
        UsmFailure.AuthPasswordTooShort => string.Format(culture, Strings.AuthPasswordTooShortFormat, UsmCredentials.MinimumPasswordLength),
        UsmFailure.PrivPasswordTooShort => string.Format(culture, Strings.PrivPasswordTooShortFormat, UsmCredentials.MinimumPasswordLength),
        UsmFailure.InvalidPrivacyParameters => Strings.ErrorInvalidPrivacyParameters,
        _ => Strings.ErrorCrypto,
    };

    private static string StatusName(long? status) =>
        status is { } value && value is >= int.MinValue and <= int.MaxValue && Enum.IsDefined((SnmpErrorStatus)value)
            ? ((SnmpErrorStatus)value).ToString()
            : status?.ToString(CultureInfo.InvariantCulture) ?? "?";

    /// <summary>From Core's English "Unexpected response: …" message, takes the detail after the colon (protocol names, OIDs, etc. aren't translated).</summary>
    private static string Detail(string message)
    {
        var colon = message.IndexOf(": ", StringComparison.Ordinal);
        return colon >= 0 ? message[(colon + 2)..] : message;
    }
}
