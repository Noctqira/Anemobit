namespace Anemobit.Core.Snmp;

/// <summary>SNMPv3 privacy protocols. AES-192 / 256 have no IETF standard and aren't supported.</summary>
public enum SnmpPrivProtocol
{
    /// <summary>DES-CBC (RFC 3414 §8).</summary>
    Des,

    /// <summary>AES-128-CFB (RFC 3826).</summary>
    Aes,
}
