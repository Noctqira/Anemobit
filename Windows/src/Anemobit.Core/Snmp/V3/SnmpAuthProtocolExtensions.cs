namespace Anemobit.Core.Snmp.V3;

internal static class SnmpAuthProtocolExtensions
{
    /// <summary>Key length = hash output length.</summary>
    public static int KeyLength(this SnmpAuthProtocol auth) => auth switch
    {
        SnmpAuthProtocol.Md5 => 16,
        SnmpAuthProtocol.Sha1 => 20,
        SnmpAuthProtocol.Sha256 => 32,
        SnmpAuthProtocol.Sha384 => 48,
        SnmpAuthProtocol.Sha512 => 64,
        _ => throw new ArgumentOutOfRangeException(nameof(auth)),
    };

    /// <summary>Length of msgAuthenticationParameters: bytes after truncating the HMAC (RFC 3414 / RFC 7860).</summary>
    public static int MacLength(this SnmpAuthProtocol auth) => auth switch
    {
        SnmpAuthProtocol.Md5 or SnmpAuthProtocol.Sha1 => 12,
        SnmpAuthProtocol.Sha256 => 24,
        SnmpAuthProtocol.Sha384 => 32,
        SnmpAuthProtocol.Sha512 => 48,
        _ => throw new ArgumentOutOfRangeException(nameof(auth)),
    };
}
