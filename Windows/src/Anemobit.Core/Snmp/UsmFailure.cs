namespace Anemobit.Core.Snmp;

/// <summary>Why SNMPv3 (USM) authentication or privacy failed; see <see cref="SnmpException.UsmFailure"/>.</summary>
public enum UsmFailure
{
    UnknownUserName,
    WrongDigest,
    DecryptionError,
    UnsupportedSecurityLevel,
    NotInTimeWindow,
    UnknownEngineId,

    /// <summary>Discovery succeeded but the encrypted request got no reply: usually the agent failed to decrypt and dropped the packet.</summary>
    NoResponseToEncryptedRequest,

    DiscoveryIncomplete,

    /// <summary>The authentication password is shorter than <see cref="UsmCredentials.MinimumPasswordLength"/> characters.</summary>
    AuthPasswordTooShort,

    /// <summary>The privacy password is shorter than <see cref="UsmCredentials.MinimumPasswordLength"/> characters.</summary>
    PrivPasswordTooShort,

    InvalidPrivacyParameters,
    CryptoFailure,
}
