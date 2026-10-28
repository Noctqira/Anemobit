using System.Text;

namespace Anemobit.Core.Snmp;

/// <summary>
/// SNMPv3 USM account. The RouterOS community name is the <see cref="UserName"/>.
/// A null <see cref="Privacy"/> means authentication without privacy (authNoPriv, RouterOS's authorized).
/// <see cref="ToString"/> excludes the user name and passwords so they don't end up in logs.
/// </summary>
public sealed record UsmCredentials(string UserName, SnmpAuthProtocol AuthProtocol, string AuthPassword, UsmPrivacy? Privacy = null)
{
    /// <summary>SNMPv3 requires passwords of at least 8 characters (RFC 3414 §11.2). Counted in UTF-8 bytes.</summary>
    public const int MinimumPasswordLength = 8;

    /// <summary>Checks password lengths; returns the reason when invalid.</summary>
    public UsmFailure? ValidationFailure =>
        Encoding.UTF8.GetByteCount(AuthPassword) < MinimumPasswordLength ? UsmFailure.AuthPasswordTooShort
        : Privacy is { } privacy && Encoding.UTF8.GetByteCount(privacy.Password) < MinimumPasswordLength ? UsmFailure.PrivPasswordTooShort
        : null;

    public override string ToString() => $"UsmCredentials {{ AuthProtocol = {AuthProtocol}, Privacy = {Privacy?.Protocol.ToString() ?? "none"} }}";
}
