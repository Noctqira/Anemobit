using Anemobit.Core.Snmp;

namespace Anemobit.Settings;

/// <summary>Connection security settings (shared by the Settings draft and the saved settings). <see cref="ToString"/> excludes sensitive data.</summary>
public sealed record SecuritySettings
{
    public SecurityMode Mode { get; init; } = SecurityMode.None;

    /// <summary>v2c community; v3 user name (the RouterOS community name). Stored in Credential Manager.</summary>
    public string Community { get; init; } = string.Empty;

    public SnmpAuthProtocol AuthProtocol { get; init; } = SnmpAuthProtocol.Sha1;

    /// <summary>Stored in Credential Manager.</summary>
    public string AuthPassword { get; init; } = string.Empty;

    public SnmpPrivProtocol PrivProtocol { get; init; } = SnmpPrivProtocol.Aes;

    /// <summary>Stored in Credential Manager.</summary>
    public string PrivPassword { get; init; } = string.Empty;

    public SnmpSecurity ToSnmpSecurity() => Mode switch
    {
        SecurityMode.None => new SnmpSecurity.Community(Community),
        _ => new SnmpSecurity.Usm(new UsmCredentials(
            Community,
            AuthProtocol,
            AuthPassword,
            Mode == SecurityMode.Private ? new UsmPrivacy(PrivProtocol, PrivPassword) : null)),
    };

    /// <summary>Why fields are incomplete or invalid; null when ready to connect.</summary>
    public SecurityProblem? Problem => string.IsNullOrEmpty(Community)
        ? (Mode == SecurityMode.None ? SecurityProblem.MissingCommunity : SecurityProblem.MissingUserName)
        : ToSnmpSecurity() is SnmpSecurity.Usm usm
            ? usm.Credentials.ValidationFailure switch
            {
                UsmFailure.AuthPasswordTooShort => SecurityProblem.AuthPasswordTooShort,
                UsmFailure.PrivPasswordTooShort => SecurityProblem.PrivPasswordTooShort,
                _ => null,
            }
            : null;

    public override string ToString() => $"SecuritySettings {{ Mode = {Mode}, AuthProtocol = {AuthProtocol}, PrivProtocol = {PrivProtocol} }}";
}
