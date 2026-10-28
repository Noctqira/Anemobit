using System.Buffers.Binary;
using System.Diagnostics.CodeAnalysis;
using System.Security.Cryptography;
using System.Text;

namespace Anemobit.Core.Snmp.V3;

/// <summary>
/// Hashing, HMAC and encryption used by USM (RFC 3414 / 3826 / 7860), all with .NET's built-in algorithms.
/// MD5, SHA1 and DES are algorithms mandated by the protocol (required when the device supports only these), not weak crypto chosen by us.
/// </summary>
[SuppressMessage("Security", "CA5350:Do Not Use Weak Cryptographic Algorithms", Justification = "Algorithm mandated by the SNMPv3 USM protocol (RFC 3414).")]
[SuppressMessage("Security", "CA5351:Do Not Use Broken Cryptographic Algorithms", Justification = "Algorithm mandated by the SNMPv3 USM protocol (RFC 3414).")]
internal static class UsmCrypto
{
    private const int PasswordExpansionLength = 1_048_576;

    // Keys

    /// <summary>RFC 3414 A.2: repeat the password to fill 1 MB and hash it to get Ku (RFC 7860's SHA-2 uses the same process).</summary>
    public static byte[] PasswordToKey(ReadOnlySpan<byte> password, SnmpAuthProtocol auth)
    {
        if (password.IsEmpty)
        {
            throw new ArgumentException("Password must not be empty.", nameof(password));
        }

        var expanded = new byte[PasswordExpansionLength];
        for (var offset = 0; offset < expanded.Length; offset += password.Length)
        {
            var count = Math.Min(password.Length, expanded.Length - offset);
            password[..count].CopyTo(expanded.AsSpan(offset));
        }
        return Hash(expanded, auth);
    }

    /// <summary>Kul = H(Ku ‖ engineID ‖ Ku).</summary>
    public static byte[] LocalizeKey(byte[] ku, byte[] engineId, SnmpAuthProtocol auth) =>
        Hash([.. ku, .. engineId, .. ku], auth);

    public static byte[] LocalizedKey(string password, byte[] engineId, SnmpAuthProtocol auth) =>
        LocalizeKey(PasswordToKey(Encoding.UTF8.GetBytes(password), auth), engineId, auth);

    public static byte[] Hash(ReadOnlySpan<byte> data, SnmpAuthProtocol auth) => auth switch
    {
        SnmpAuthProtocol.Md5 => MD5.HashData(data),
        SnmpAuthProtocol.Sha1 => SHA1.HashData(data),
        SnmpAuthProtocol.Sha256 => SHA256.HashData(data),
        SnmpAuthProtocol.Sha384 => SHA384.HashData(data),
        SnmpAuthProtocol.Sha512 => SHA512.HashData(data),
        _ => throw new ArgumentOutOfRangeException(nameof(auth)),
    };

    // Authentication

    /// <summary>HMAC truncated to <see cref="SnmpAuthProtocolExtensions.MacLength"/> bytes.</summary>
    public static byte[] AuthenticationCode(byte[] key, byte[] message, SnmpAuthProtocol auth)
    {
        var mac = auth switch
        {
            SnmpAuthProtocol.Md5 => HMACMD5.HashData(key, message),
            SnmpAuthProtocol.Sha1 => HMACSHA1.HashData(key, message),
            SnmpAuthProtocol.Sha256 => HMACSHA256.HashData(key, message),
            SnmpAuthProtocol.Sha384 => HMACSHA384.HashData(key, message),
            SnmpAuthProtocol.Sha512 => HMACSHA512.HashData(key, message),
            _ => throw new ArgumentOutOfRangeException(nameof(auth)),
        };
        return mac[..auth.MacLength()];
    }

    // Privacy

    /// <summary>Encrypts the ScopedPDU and returns the ciphertext and msgPrivacyParameters (8-byte salt).</summary>
    public static (byte[] Ciphertext, byte[] PrivParams) Encrypt(
        byte[] plaintext, SnmpPrivProtocol privacy, byte[] key, uint engineBoots, uint engineTime, ulong salt)
    {
        switch (privacy)
        {
            case SnmpPrivProtocol.Des:
            {
                byte[] privParams = [.. BigEndian(engineBoots), .. BigEndian((uint)salt)];
                var padded = plaintext.Length % 8 == 0
                    ? plaintext
                    : [.. plaintext, .. new byte[8 - (plaintext.Length % 8)]];
                return (DesCbc(encrypt: true, key[..8], DesIv(key, privParams), padded), privParams);
            }
            case SnmpPrivProtocol.Aes:
            {
                var privParams = BigEndian(salt);
                return (AesCfb128(encrypt: true, key[..16], AesIv(engineBoots, engineTime, privParams), plaintext), privParams);
            }
            default:
                throw new ArgumentOutOfRangeException(nameof(privacy));
        }
    }

    /// <summary>Decrypts the ScopedPDU. DES results may carry padding bytes.</summary>
    /// <exception cref="SnmpException">privParams has the wrong length or the ciphertext isn't a multiple of the block size (<see cref="UsmFailure.InvalidPrivacyParameters"/>).</exception>
    public static byte[] Decrypt(
        byte[] ciphertext, SnmpPrivProtocol privacy, byte[] key, uint engineBoots, uint engineTime, byte[] privParams)
    {
        if (privParams.Length != 8)
        {
            throw SnmpException.Authentication(UsmFailure.InvalidPrivacyParameters);
        }
        switch (privacy)
        {
            case SnmpPrivProtocol.Des:
                if (ciphertext.Length % 8 != 0)
                {
                    throw SnmpException.Authentication(UsmFailure.InvalidPrivacyParameters);
                }
                return DesCbc(encrypt: false, key[..8], DesIv(key, privParams), ciphertext);
            case SnmpPrivProtocol.Aes:
                return AesCfb128(encrypt: false, key[..16], AesIv(engineBoots, engineTime, privParams), ciphertext);
            default:
                throw new ArgumentOutOfRangeException(nameof(privacy));
        }
    }

    /// <summary>DES-CBC without padding (data length must be a multiple of 8).</summary>
    public static byte[] DesCbc(bool encrypt, byte[] key, byte[] iv, byte[] data)
    {
        try
        {
            using var des = DES.Create();
            des.Key = key;
            return encrypt
                ? des.EncryptCbc(data, iv, PaddingMode.None)
                : des.DecryptCbc(data, iv, PaddingMode.None);
        }
        catch (CryptographicException error)
        {
            // E.g. the localized key happens to be a DES weak key (extremely unlikely).
            throw SnmpException.Authentication(UsmFailure.CryptoFailure, error);
        }
    }

    /// <summary>
    /// AES-128-CFB (128-bit feedback); output is the same length as input.
    /// .NET's CFB processes whole blocks, so a partial block is zero-padded and the result truncated; earlier CFB output doesn't depend on later data.
    /// </summary>
    public static byte[] AesCfb128(bool encrypt, byte[] key, byte[] iv, byte[] data)
    {
        try
        {
            using var aes = Aes.Create();
            aes.Key = key;
            var padded = data.Length % 16 == 0 ? data : [.. data, .. new byte[16 - (data.Length % 16)]];
            var output = encrypt
                ? aes.EncryptCfb(padded, iv, PaddingMode.None, feedbackSizeInBits: 128)
                : aes.DecryptCfb(padded, iv, PaddingMode.None, feedbackSizeInBits: 128);
            return output[..data.Length];
        }
        catch (CryptographicException error)
        {
            throw SnmpException.Authentication(UsmFailure.CryptoFailure, error);
        }
    }

    /// <summary>RFC 3414 §8.1.1.1: IV = pre-IV (last 8 bytes of the key) XOR salt.</summary>
    private static byte[] DesIv(byte[] key, byte[] privParams)
    {
        var iv = new byte[8];
        for (var i = 0; i < 8; i++)
        {
            iv[i] = (byte)(key[8 + i] ^ privParams[i]);
        }
        return iv;
    }

    /// <summary>RFC 3826 §3.1.2.1: IV = engineBoots ‖ engineTime ‖ salt.</summary>
    private static byte[] AesIv(uint engineBoots, uint engineTime, byte[] privParams) =>
        [.. BigEndian(engineBoots), .. BigEndian(engineTime), .. privParams];

    private static byte[] BigEndian(uint value)
    {
        var bytes = new byte[4];
        BinaryPrimitives.WriteUInt32BigEndian(bytes, value);
        return bytes;
    }

    private static byte[] BigEndian(ulong value)
    {
        var bytes = new byte[8];
        BinaryPrimitives.WriteUInt64BigEndian(bytes, value);
        return bytes;
    }
}
