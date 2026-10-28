using Anemobit.Core.Snmp;
using Anemobit.Core.Snmp.V3;

namespace Anemobit.Core.Tests.Snmp.V3;

public class UsmCryptoTests
{
    private static readonly byte[] RfcEngineId = Hex("00 00 00 00 00 00 00 00 00 00 00 02");

    // Key localization

    [Fact]
    public void PasswordToKey_Md5_MatchesRfc3414A31()
    {
        var ku = UsmCrypto.PasswordToKey("maplesyrup"u8, SnmpAuthProtocol.Md5);
        Assert.Equal(Hex("9f af 32 83 88 4e 92 83 4e bc 98 47 d8 ed d9 63"), ku);
        Assert.Equal(Hex("52 6f 5e ed 9f cc e2 6f 89 64 c2 93 07 87 d8 2b"), UsmCrypto.LocalizeKey(ku, RfcEngineId, SnmpAuthProtocol.Md5));
    }

    [Fact]
    public void PasswordToKey_Sha1_MatchesRfc3414A32()
    {
        var ku = UsmCrypto.PasswordToKey("maplesyrup"u8, SnmpAuthProtocol.Sha1);
        Assert.Equal(Hex("9f b5 cc 03 81 49 7b 37 93 52 89 39 ff 78 8d 5d 79 14 52 11"), ku);
        Assert.Equal(
            Hex("66 95 fe bc 92 88 e3 62 82 23 5f c7 15 1f 12 84 97 b3 8f 3f"),
            UsmCrypto.LocalizeKey(ku, RfcEngineId, SnmpAuthProtocol.Sha1));
    }

    // RFC 7860 has no key localization test vectors; SHA-2 uses the same RFC 3414 A.2 process (only the hash differs).
    // Expected values come from an independent implementation (Python hashlib) whose MD5 / SHA1 results match the RFC 3414 A.3 vectors above.
    [Theory]
    [InlineData(
        SnmpAuthProtocol.Sha256,
        "ab51014d1e077f6017df2b12bee5f5aa72993177e9bb569c4dff5a4ca0b4afac",
        "8982e0e549e866db361a6b625d84cccc11162d453ee8ce3a6445c2d6776f0f8b")]
    [InlineData(
        SnmpAuthProtocol.Sha384,
        "e06eccdf2c68a06ed034723c9c26e0db3b669e1e2efed49150b55377a2e98f383c86fb836857444654b287c93f51ff64",
        "3b298f16164a11184279d5432bf169e2d2a48307de02b3d3f7e2b4f36eb6f0455a53689a3937eea07319a633d2ccba78")]
    [InlineData(
        SnmpAuthProtocol.Sha512,
        "7e4396de5aadc77be853819b98c9406265b3a9c37cc3176569847a4e4f6fba63dd3a73d04924d31a63f95a601f9385af6be4ed1b37f87d040f7c6ed6f8d38a91",
        "22a5a36cedfcc085807a128d7bc6c2382167ad6c0dbc5fdff856740f3d84c099ad1ea87a8db096714d9788bd544047c9021e4229ce27e4c0a69250adfcffbb0b")]
    public void PasswordToKey_Sha2_MatchesIndependentImplementation(SnmpAuthProtocol auth, string ku, string kul)
    {
        var key = UsmCrypto.PasswordToKey("maplesyrup"u8, auth);
        Assert.Equal(Hex(ku), key);
        Assert.Equal(auth.KeyLength(), key.Length);
        Assert.Equal(Hex(kul), UsmCrypto.LocalizeKey(key, RfcEngineId, auth));
    }

    [Fact]
    public void LocalizedKey_Password_CombinesBothSteps()
    {
        Assert.Equal(
            Hex("52 6f 5e ed 9f cc e2 6f 89 64 c2 93 07 87 d8 2b"),
            UsmCrypto.LocalizedKey("maplesyrup", RfcEngineId, SnmpAuthProtocol.Md5));
    }

    // HMAC

    [Fact]
    public void AuthenticationCode_Md5_IsRfc2202TruncatedTo12Bytes()
    {
        var mac = UsmCrypto.AuthenticationCode(Repeat(0x0b, 16), "Hi There"u8.ToArray(), SnmpAuthProtocol.Md5);
        Assert.Equal(Hex("9294727a3638bb1c13f48ef8158bfc9d")[..12], mac);
    }

    [Fact]
    public void AuthenticationCode_Sha1_IsRfc2202TruncatedTo12Bytes()
    {
        var mac = UsmCrypto.AuthenticationCode(Repeat(0x0b, 20), "Hi There"u8.ToArray(), SnmpAuthProtocol.Sha1);
        Assert.Equal(Hex("b617318655057264e28bc0b6fb378c8ef146be00")[..12], mac);
    }

    // RFC 4231 test case 1 (key = 0x0b × 20, "Hi There"), truncated to 24 / 32 / 48 bytes per RFC 7860.
    [Theory]
    [InlineData(SnmpAuthProtocol.Sha256, 24, "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7")]
    [InlineData(SnmpAuthProtocol.Sha384, 32, "afd03944d84895626b0825f4ab46907f15f9dadbe4101ec682aa034c7cebc59cfaea9ea9076ede7f4af152e8b2fa9cb6")]
    [InlineData(SnmpAuthProtocol.Sha512, 48, "87aa7cdea5ef619d4ff0b4241a1d6cb02379f4e2ce4ec2787ad0b30545e17cdedaa833b7d6b8a702038b274eaea3f4e4be9d914eeb61f1702e696c203a126854")]
    public void AuthenticationCode_Sha2_IsRfc4231Case1Truncated(SnmpAuthProtocol auth, int length, string full)
    {
        var mac = UsmCrypto.AuthenticationCode(Repeat(0x0b, 20), "Hi There"u8.ToArray(), auth);
        Assert.Equal(length, auth.MacLength());
        Assert.Equal(Hex(full)[..length], mac);
    }

    // RFC 4231 test case 2 (key = "Jefe", "what do ya want for nothing?")
    [Theory]
    [InlineData(SnmpAuthProtocol.Sha256, "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843")]
    [InlineData(SnmpAuthProtocol.Sha384, "af45d2e376484031617f78d2b58a6b1b9c7ef464f5a01b47e42ec3736322445e8e2240ca5e69e2c78b3239ecfab21649")]
    [InlineData(SnmpAuthProtocol.Sha512, "164b7a7bfcf819e2e395fbe73b56e0a387bd64222e831fd610270cd7ea2505549758bf75c05a994a6d034f65f8f0e6fdcaeab1a34d4a6b4b636e070a38bce737")]
    public void AuthenticationCode_Sha2_IsRfc4231Case2Truncated(SnmpAuthProtocol auth, string full)
    {
        var mac = UsmCrypto.AuthenticationCode("Jefe"u8.ToArray(), "what do ya want for nothing?"u8.ToArray(), auth);
        Assert.Equal(Hex(full)[..auth.MacLength()], mac);
    }

    // Encryption and decryption

    [Fact]
    public void DesCbc_Fips81Example_MatchesKnownCiphertext()
    {
        var plaintext = "Now is the time for all "u8.ToArray();
        var key = Hex("0123456789abcdef");
        var iv = Hex("1234567890abcdef");

        var ciphertext = UsmCrypto.DesCbc(encrypt: true, key, iv, plaintext);

        Assert.Equal(Hex("e5c7cdde872bf27c 43e934008c389c0f 683788499a7c05f6"), ciphertext);
        Assert.Equal(plaintext, UsmCrypto.DesCbc(encrypt: false, key, iv, ciphertext));
    }

    [Fact]
    public void AesCfb128_NistSp80038aF313_MatchesKnownCiphertext()
    {
        var key = Hex("2b7e151628aed2a6abf7158809cf4f3c");
        var iv = Hex("000102030405060708090a0b0c0d0e0f");
        var plaintext = Hex("6bc1bee22e409f96e93d7e117393172a ae2d8a571e03ac9c9eb76fac45af8e51");

        var ciphertext = UsmCrypto.AesCfb128(encrypt: true, key, iv, plaintext);

        Assert.Equal(Hex("3b3fd92eb72dad20333449f8e83cfb4a c8a64537a0b3a93fcde3cdad9f1ce58b"), ciphertext);
        Assert.Equal(plaintext, UsmCrypto.AesCfb128(encrypt: false, key, iv, ciphertext));
    }

    [Fact]
    public void AesCfb128_PartialBlock_KeepsLengthAndMatchesFullBlockPrefix()
    {
        var key = Hex("2b7e151628aed2a6abf7158809cf4f3c");
        var iv = Hex("000102030405060708090a0b0c0d0e0f");
        var plaintext = Hex("6bc1bee22e409f96e93d7e117393172a ae2d8a571e");

        var ciphertext = UsmCrypto.AesCfb128(encrypt: true, key, iv, plaintext);

        // Earlier CFB ciphertext doesn't depend on later data: it should be the first 21 bytes of the NIST vector.
        Assert.Equal(Hex("3b3fd92eb72dad20333449f8e83cfb4a c8a64537a0")[..21], ciphertext);
        Assert.Equal(plaintext, UsmCrypto.AesCfb128(encrypt: false, key, iv, ciphertext));
    }

    [Theory]
    [MemberData(nameof(AllAuthProtocols))]
    public void Encrypt_Des_PadsToBlockAndRoundTrips(SnmpAuthProtocol auth)
    {
        var key = UsmCrypto.LocalizedKey("privpass12", [1, 2, 3, 4, 5], auth);
        byte[] scopedPdu = [.. Enumerable.Range(0, 21).Select(i => (byte)i)];

        var (ciphertext, privParams) = UsmCrypto.Encrypt(scopedPdu, SnmpPrivProtocol.Des, key, 7, 1234, 0x0102_0304_0506_0708);

        Assert.Equal(24, ciphertext.Length);
        Assert.Equal(8, privParams.Length);
        // DES salt = engineBoots ‖ low 32 bits of the counter
        Assert.Equal([0, 0, 0, 7, 5, 6, 7, 8], privParams);
        var decrypted = UsmCrypto.Decrypt(ciphertext, SnmpPrivProtocol.Des, key, 7, 1234, privParams);
        Assert.Equal(scopedPdu, decrypted[..21]);
    }

    [Theory]
    [MemberData(nameof(AllAuthProtocols))]
    public void Encrypt_Aes_KeepsLengthAndRoundTrips(SnmpAuthProtocol auth)
    {
        var key = UsmCrypto.LocalizedKey("privpass12", [1, 2, 3, 4, 5], auth);
        byte[] scopedPdu = [.. Enumerable.Range(0, 37).Select(i => (byte)i)];

        var (ciphertext, privParams) = UsmCrypto.Encrypt(scopedPdu, SnmpPrivProtocol.Aes, key, 7, 1234, 0x0102_0304_0506_0708);

        Assert.Equal(37, ciphertext.Length);
        Assert.Equal([1, 2, 3, 4, 5, 6, 7, 8], privParams);
        Assert.Equal(scopedPdu, UsmCrypto.Decrypt(ciphertext, SnmpPrivProtocol.Aes, key, 7, 1234, privParams));
    }

    [Fact]
    public void Decrypt_DesCiphertextNotBlockMultiple_ThrowsInvalidPrivacyParameters()
    {
        var error = Assert.Throws<SnmpException>(() =>
            UsmCrypto.Decrypt([1, 2, 3], SnmpPrivProtocol.Des, Repeat(1, 16), 0, 0, new byte[8]));
        Assert.Equal(UsmFailure.InvalidPrivacyParameters, error.UsmFailure);
    }

    [Fact]
    public void Decrypt_PrivParamsNotEightBytes_ThrowsInvalidPrivacyParameters()
    {
        var error = Assert.Throws<SnmpException>(() =>
            UsmCrypto.Decrypt(new byte[8], SnmpPrivProtocol.Aes, Repeat(1, 16), 0, 0, [1, 2]));
        Assert.Equal(UsmFailure.InvalidPrivacyParameters, error.UsmFailure);
    }

    public static TheoryData<SnmpAuthProtocol> AllAuthProtocols => [.. Enum.GetValues<SnmpAuthProtocol>()];

    internal static byte[] Hex(string text) => Convert.FromHexString(text.Replace(" ", string.Empty, StringComparison.Ordinal));

    private static byte[] Repeat(byte value, int count) => Enumerable.Repeat(value, count).ToArray();
}
