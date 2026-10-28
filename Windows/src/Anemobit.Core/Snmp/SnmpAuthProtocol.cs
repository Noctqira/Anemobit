namespace Anemobit.Core.Snmp;

/// <summary>
/// SNMPv3 authentication protocols: HMAC-MD5-96 / HMAC-SHA1-96 (RFC 3414) and HMAC-SHA-2 (RFC 7860).
/// The Windows version doesn't support SHA-224 (.NET has no SHA-224, and the user chose not to implement it).
/// </summary>
public enum SnmpAuthProtocol
{
    Md5,
    Sha1,

    /// <summary>usmHMAC192SHA256AuthProtocol.</summary>
    Sha256,

    /// <summary>usmHMAC256SHA384AuthProtocol.</summary>
    Sha384,

    /// <summary>usmHMAC384SHA512AuthProtocol.</summary>
    Sha512,
}
