namespace Anemobit.Core.Snmp;

/// <summary>SNMPv3 privacy settings. <see cref="ToString"/> excludes the password.</summary>
public sealed record UsmPrivacy(SnmpPrivProtocol Protocol, string Password)
{
    public override string ToString() => $"UsmPrivacy {{ Protocol = {Protocol} }}";
}
