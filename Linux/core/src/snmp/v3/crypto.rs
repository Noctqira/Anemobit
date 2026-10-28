//! Hashing, HMAC and encryption used by USM (RFC 3414 / 3826 / 7860), implemented with RustCrypto.

use aes::Aes128;
use des::Des;
use des::cipher::{Block, BlockCipherDecrypt, BlockCipherEncrypt, KeyInit, KeyIvInit};
use hmac::{EagerHash, Hmac, Mac};
use md5::Md5;
use sha1::Sha1;
use sha2::{Digest, Sha224, Sha256, Sha384, Sha512};

use crate::{SnmpError, UsmFailure};

/// Picks the hash function for the authentication protocol and calls the generic function `$f::<Hash>(args…)`.
macro_rules! with_hash {
    ($auth:expr, $f:ident($($arg:expr),*)) => {
        match $auth {
            AuthProtocol::Md5 => $f::<Md5>($($arg),*),
            AuthProtocol::Sha1 => $f::<Sha1>($($arg),*),
            AuthProtocol::Sha224 => $f::<Sha224>($($arg),*),
            AuthProtocol::Sha256 => $f::<Sha256>($($arg),*),
            AuthProtocol::Sha384 => $f::<Sha384>($($arg),*),
            AuthProtocol::Sha512 => $f::<Sha512>($($arg),*),
        }
    };
}

/// SNMPv3 authentication protocols: HMAC-MD5-96 / HMAC-SHA1-96 (RFC 3414) and HMAC-SHA-2 (RFC 7860).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AuthProtocol {
    Md5,
    Sha1,
    /// usmHMAC128SHA224AuthProtocol
    Sha224,
    /// usmHMAC192SHA256AuthProtocol
    Sha256,
    /// usmHMAC256SHA384AuthProtocol
    Sha384,
    /// usmHMAC384SHA512AuthProtocol
    Sha512,
}

impl AuthProtocol {
    pub const ALL: [AuthProtocol; 6] = [
        Self::Md5,
        Self::Sha1,
        Self::Sha224,
        Self::Sha256,
        Self::Sha384,
        Self::Sha512,
    ];

    /// Length of msgAuthenticationParameters: bytes after truncating the HMAC.
    pub(crate) fn mac_length(self) -> usize {
        match self {
            Self::Md5 | Self::Sha1 => 12,
            Self::Sha224 => 16,
            Self::Sha256 => 24,
            Self::Sha384 => 32,
            Self::Sha512 => 48,
        }
    }
}

/// SNMPv3 privacy protocols.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PrivProtocol {
    /// DES-CBC (RFC 3414 §8)
    Des,
    /// AES-128-CFB (RFC 3826)
    Aes,
}

/// RFC 3414 A.2: repeat the password to fill 1 MB and hash it to get Ku (RFC 7860's SHA-2 uses the same process).
pub(crate) fn password_to_key(password: &[u8], auth: AuthProtocol) -> Vec<u8> {
    assert!(!password.is_empty(), "password must not be empty");
    with_hash!(auth, expand_and_hash(password))
}

fn expand_and_hash<D: Digest>(password: &[u8]) -> Vec<u8> {
    const TOTAL: usize = 1_048_576;
    // Feed the hash 64 bytes at a time; no need to actually allocate 1 MB.
    let mut hasher = D::new();
    let mut chunk = [0u8; 64];
    let mut position = 0;
    for _ in 0..TOTAL / chunk.len() {
        for byte in &mut chunk {
            *byte = password[position % password.len()];
            position += 1;
        }
        hasher.update(chunk);
    }
    hasher.finalize().to_vec()
}

/// Kul = H(Ku ‖ engineID ‖ Ku)
pub(crate) fn localize_key(ku: &[u8], engine_id: &[u8], auth: AuthProtocol) -> Vec<u8> {
    with_hash!(auth, hash_parts(&[ku, engine_id, ku]))
}

fn hash_parts<D: Digest>(parts: &[&[u8]]) -> Vec<u8> {
    let mut hasher = D::new();
    for part in parts {
        hasher.update(part);
    }
    hasher.finalize().to_vec()
}

pub(crate) fn localized_key(password: &str, engine_id: &[u8], auth: AuthProtocol) -> Vec<u8> {
    localize_key(&password_to_key(password.as_bytes(), auth), engine_id, auth)
}

/// HMAC truncated to `auth.mac_length()` bytes.
pub(crate) fn authentication_code(key: &[u8], message: &[u8], auth: AuthProtocol) -> Vec<u8> {
    let mut mac = with_hash!(auth, hmac(key, message));
    mac.truncate(auth.mac_length());
    mac
}

fn hmac<D: EagerHash>(key: &[u8], message: &[u8]) -> Vec<u8> {
    let mut mac =
        <Hmac<D> as KeyInit>::new_from_slice(key).expect("HMAC accepts keys of any length");
    mac.update(message);
    mac.finalize().into_bytes().to_vec()
}

/// Encrypts the ScopedPDU and returns the ciphertext and msgPrivacyParameters (8-byte salt).
///
/// `key` is the localized privacy key (at least 16 bytes): DES uses the first 8 bytes as the key and the last 8 for the IV;
/// AES-128 uses the first 16 bytes.
pub(crate) fn encrypt(
    plaintext: &[u8],
    privacy: PrivProtocol,
    key: &[u8],
    engine_boots: u32,
    engine_time: u32,
    salt: u64,
) -> (Vec<u8>, [u8; 8]) {
    match privacy {
        PrivProtocol::Des => {
            // salt = engineBoots ‖ low 32 bits of the counter (RFC 3414 §8.1.1.1)
            let mut priv_params = [0; 8];
            priv_params[..4].copy_from_slice(&engine_boots.to_be_bytes());
            priv_params[4..].copy_from_slice(&(salt as u32).to_be_bytes());
            let mut padded = plaintext.to_vec();
            padded.resize(plaintext.len().next_multiple_of(8), 0);
            (
                des_cbc(true, &key[..8], &des_iv(key, &priv_params), &padded),
                priv_params,
            )
        }
        PrivProtocol::Aes => {
            let priv_params = salt.to_be_bytes();
            let iv = aes_iv(engine_boots, engine_time, &priv_params);
            (aes_cfb128(true, &key[..16], &iv, plaintext), priv_params)
        }
    }
}

pub(crate) fn decrypt(
    ciphertext: &[u8],
    privacy: PrivProtocol,
    key: &[u8],
    engine_boots: u32,
    engine_time: u32,
    priv_params: &[u8],
) -> Result<Vec<u8>, SnmpError> {
    let invalid = SnmpError::Authentication(UsmFailure::InvalidPrivacyParameters);
    let priv_params: &[u8; 8] = priv_params.try_into().map_err(|_| invalid.clone())?;
    match privacy {
        PrivProtocol::Des => {
            if !ciphertext.len().is_multiple_of(8) {
                return Err(invalid);
            }
            Ok(des_cbc(
                false,
                &key[..8],
                &des_iv(key, priv_params),
                ciphertext,
            ))
        }
        PrivProtocol::Aes => {
            let iv = aes_iv(engine_boots, engine_time, priv_params);
            Ok(aes_cfb128(false, &key[..16], &iv, ciphertext))
        }
    }
}

/// DES IV = pre-IV (bytes 9–16 of the key) XOR salt.
fn des_iv(key: &[u8], priv_params: &[u8; 8]) -> [u8; 8] {
    std::array::from_fn(|i| key[8 + i] ^ priv_params[i])
}

/// AES IV = engineBoots ‖ engineTime ‖ salt (RFC 3826 §3.1.2.1).
fn aes_iv(engine_boots: u32, engine_time: u32, priv_params: &[u8; 8]) -> [u8; 16] {
    let mut iv = [0; 16];
    iv[..4].copy_from_slice(&engine_boots.to_be_bytes());
    iv[4..8].copy_from_slice(&engine_time.to_be_bytes());
    iv[8..].copy_from_slice(priv_params);
    iv
}

/// DES-CBC; the length of `data` must be a multiple of 8.
fn des_cbc(encrypt: bool, key: &[u8], iv: &[u8], data: &[u8]) -> Vec<u8> {
    // RustCrypto's CBC is in a separate crate (cbc); CBC is just XOR per block then encrypt, so it's built directly on the DES block cipher.
    let cipher = Des::new_from_slice(key).expect("DES key is 8 bytes");
    let mut previous = Block::<Des>::default();
    previous.copy_from_slice(iv);
    let mut output = Vec::with_capacity(data.len());
    // The caller guarantees a multiple of 8, so as_chunks leaves no remainder.
    for chunk in data.as_chunks::<8>().0 {
        let mut block = Block::<Des>::from(*chunk);
        if encrypt {
            block
                .iter_mut()
                .zip(previous.iter())
                .for_each(|(b, p)| *b ^= p);
            cipher.encrypt_block(&mut block);
            previous = block;
        } else {
            let input = block;
            cipher.decrypt_block(&mut block);
            block
                .iter_mut()
                .zip(previous.iter())
                .for_each(|(b, p)| *b ^= p);
            previous = input;
        }
        output.extend_from_slice(&block);
    }
    output
}

/// AES-128-CFB (128-bit feedback); no block padding needed.
fn aes_cfb128(encrypt: bool, key: &[u8], iv: &[u8], data: &[u8]) -> Vec<u8> {
    let mut buffer = data.to_vec();
    if encrypt {
        cfb_mode::Encryptor::<Aes128>::new_from_slices(key, iv)
            .expect("AES-128 key and IV are 16 bytes")
            .encrypt(&mut buffer);
    } else {
        cfb_mode::Decryptor::<Aes128>::new_from_slices(key, iv)
            .expect("AES-128 key and IV are 16 bytes")
            .decrypt(&mut buffer);
    }
    buffer
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::hex;

    const ENGINE_ID: [u8; 12] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2];

    #[test]
    fn md5_password_to_key_rfc_3414_a_3_1() {
        let ku = password_to_key(b"maplesyrup", AuthProtocol::Md5);
        assert_eq!(ku, hex("9f af 32 83 88 4e 92 83 4e bc 98 47 d8 ed d9 63"));
        assert_eq!(
            localize_key(&ku, &ENGINE_ID, AuthProtocol::Md5),
            hex("52 6f 5e ed 9f cc e2 6f 89 64 c2 93 07 87 d8 2b")
        );
    }

    #[test]
    fn sha1_password_to_key_rfc_3414_a_3_2() {
        let ku = password_to_key(b"maplesyrup", AuthProtocol::Sha1);
        assert_eq!(
            ku,
            hex("9f b5 cc 03 81 49 7b 37 93 52 89 39 ff 78 8d 5d 79 14 52 11")
        );
        assert_eq!(
            localize_key(&ku, &ENGINE_ID, AuthProtocol::Sha1),
            hex("66 95 fe bc 92 88 e3 62 82 23 5f c7 15 1f 12 84 97 b3 8f 3f")
        );
    }

    /// RFC 7860 has no key localization test vectors; SHA-2 uses the same RFC 3414 A.2 process (only the hash differs).
    /// Expected values come from an independent implementation (Python hashlib) whose MD5 / SHA1 results match the RFC 3414 A.3 vectors
    /// (the same vectors as the Swift version).
    #[test]
    fn sha2_password_to_key() {
        let cases = [
            (
                AuthProtocol::Sha224,
                "282a5867ee9aac639ad59df9572c7d3ac0fbc13a905b6df07dbbf00b",
                "0bd8827c6e29f8065e08e09237f177e410f69b90e1782be682075674",
            ),
            (
                AuthProtocol::Sha256,
                "ab51014d1e077f6017df2b12bee5f5aa72993177e9bb569c4dff5a4ca0b4afac",
                "8982e0e549e866db361a6b625d84cccc11162d453ee8ce3a6445c2d6776f0f8b",
            ),
            (
                AuthProtocol::Sha384,
                "e06eccdf2c68a06ed034723c9c26e0db3b669e1e2efed49150b55377a2e98f383c86fb836857444654b287c93f51ff64",
                "3b298f16164a11184279d5432bf169e2d2a48307de02b3d3f7e2b4f36eb6f0455a53689a3937eea07319a633d2ccba78",
            ),
            (
                AuthProtocol::Sha512,
                "7e4396de5aadc77be853819b98c9406265b3a9c37cc3176569847a4e4f6fba63dd3a73d04924d31a63f95a601f9385af6be4ed1b37f87d040f7c6ed6f8d38a91",
                "22a5a36cedfcc085807a128d7bc6c2382167ad6c0dbc5fdff856740f3d84c099ad1ea87a8db096714d9788bd544047c9021e4229ce27e4c0a69250adfcffbb0b",
            ),
        ];
        for (auth, ku, kul) in cases {
            let key = password_to_key(b"maplesyrup", auth);
            assert_eq!(key, hex(ku), "{auth:?}");
            assert_eq!(localize_key(&key, &ENGINE_ID, auth), hex(kul), "{auth:?}");
        }
    }

    #[test]
    fn localized_key_combines_both_steps() {
        assert_eq!(
            localized_key("maplesyrup", &ENGINE_ID, AuthProtocol::Md5),
            hex("52 6f 5e ed 9f cc e2 6f 89 64 c2 93 07 87 d8 2b")
        );
    }

    #[test]
    fn key_length_is_hash_output_length() {
        // RFC 7860: key length = hash output length.
        let lengths: Vec<_> = AuthProtocol::ALL
            .iter()
            .map(|&a| password_to_key(b"maplesyrup", a).len())
            .collect();
        assert_eq!(lengths, [16, 20, 28, 32, 48, 64]);
    }

    /// RFC 2202 test case 1, first 12 bytes.
    #[test]
    fn hmac_md5_96_and_sha1_96() {
        let mac = authentication_code(&[0x0b; 16], b"Hi There", AuthProtocol::Md5);
        assert_eq!(mac, hex("9294727a3638bb1c13f48ef8158bfc9d")[..12]);
        let mac = authentication_code(&[0x0b; 20], b"Hi There", AuthProtocol::Sha1);
        assert_eq!(mac, hex("b617318655057264e28bc0b6fb378c8ef146be00")[..12]);
    }

    /// RFC 4231 test case 1 (key = 0x0b × 20, "Hi There"), truncated to 16 / 24 / 32 / 48 bytes per RFC 7860.
    #[test]
    fn hmac_sha2_rfc_4231_test_case_1() {
        let cases = [
            (
                AuthProtocol::Sha224,
                16,
                "896fb1128abbdf196832107cd49df33f47b4b1169912ba4f53684b22",
            ),
            (
                AuthProtocol::Sha256,
                24,
                "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7",
            ),
            (
                AuthProtocol::Sha384,
                32,
                "afd03944d84895626b0825f4ab46907f15f9dadbe4101ec682aa034c7cebc59cfaea9ea9076ede7f4af152e8b2fa9cb6",
            ),
            (
                AuthProtocol::Sha512,
                48,
                "87aa7cdea5ef619d4ff0b4241a1d6cb02379f4e2ce4ec2787ad0b30545e17cdedaa833b7d6b8a702038b274eaea3f4e4be9d914eeb61f1702e696c203a126854",
            ),
        ];
        for (auth, length, full) in cases {
            assert_eq!(auth.mac_length(), length, "{auth:?}");
            let mac = authentication_code(&[0x0b; 20], b"Hi There", auth);
            assert_eq!(mac, hex(full)[..length], "{auth:?}");
        }
        assert_eq!(AuthProtocol::Md5.mac_length(), 12);
        assert_eq!(AuthProtocol::Sha1.mac_length(), 12);
    }

    /// RFC 4231 test case 2 (key = "Jefe", "what do ya want for nothing?").
    #[test]
    fn hmac_sha2_rfc_4231_test_case_2() {
        let cases = [
            (
                AuthProtocol::Sha224,
                "a30e01098bc6dbbf45690f3a7e9e6d0f8bbea2a39e6148008fd05e44",
            ),
            (
                AuthProtocol::Sha256,
                "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843",
            ),
            (
                AuthProtocol::Sha384,
                "af45d2e376484031617f78d2b58a6b1b9c7ef464f5a01b47e42ec3736322445e8e2240ca5e69e2c78b3239ecfab21649",
            ),
            (
                AuthProtocol::Sha512,
                "164b7a7bfcf819e2e395fbe73b56e0a387bd64222e831fd610270cd7ea2505549758bf75c05a994a6d034f65f8f0e6fdcaeab1a34d4a6b4b636e070a38bce737",
            ),
        ];
        for (auth, full) in cases {
            let mac = authentication_code(b"Jefe", b"what do ya want for nothing?", auth);
            assert_eq!(mac, hex(full)[..auth.mac_length()], "{auth:?}");
        }
    }

    /// FIPS 81 DES-CBC example.
    #[test]
    fn des_cbc_fips_81_vector() {
        let key = hex("0123456789abcdef");
        let iv = hex("1234567890abcdef");
        let plaintext = b"Now is the time for all ";
        let ciphertext = des_cbc(true, &key, &iv, plaintext);
        assert_eq!(
            ciphertext,
            hex("e5c7cdde872bf27c 43e934008c389c0f 683788499a7c05f6")
        );
        assert_eq!(des_cbc(false, &key, &iv, &ciphertext), plaintext);
    }

    /// NIST SP 800-38A F.3.13 CFB128-AES128.
    #[test]
    fn aes_cfb128_nist_vector() {
        let key = hex("2b7e151628aed2a6abf7158809cf4f3c");
        let iv = hex("000102030405060708090a0b0c0d0e0f");
        let plaintext = hex("6bc1bee22e409f96e93d7e117393172a ae2d8a571e03ac9c9eb76fac45af8e51");
        let ciphertext = aes_cfb128(true, &key, &iv, &plaintext);
        assert_eq!(
            ciphertext,
            hex("3b3fd92eb72dad20333449f8e83cfb4a c8a64537a0b3a93fcde3cdad9f1ce58b")
        );
        assert_eq!(aes_cfb128(false, &key, &iv, &ciphertext), plaintext);
    }

    /// With less than one block, the result equals the first bytes of encrypting a full block (a property of CFB).
    #[test]
    fn aes_cfb_handles_partial_block() {
        let key = hex("2b7e151628aed2a6abf7158809cf4f3c");
        let iv = hex("000102030405060708090a0b0c0d0e0f");
        let plaintext = hex("6bc1bee22e409f96e93d7e117393172a ae");
        let ciphertext = aes_cfb128(true, &key, &iv, &plaintext);
        assert_eq!(ciphertext, hex("3b3fd92eb72dad20333449f8e83cfb4a c8"));
        assert_eq!(aes_cfb128(false, &key, &iv, &ciphertext), plaintext);
    }

    #[test]
    fn usm_des_round_trip_pads_to_block() {
        for auth in AuthProtocol::ALL {
            let key = localized_key("privpass12", &[1, 2, 3, 4, 5], auth);
            let scoped_pdu: Vec<u8> = (0..21).collect();
            let (ciphertext, priv_params) = encrypt(
                &scoped_pdu,
                PrivProtocol::Des,
                &key,
                7,
                1234,
                0x0102_0304_0506_0708,
            );
            assert_eq!(ciphertext.len(), 24);
            // DES salt = engineBoots ‖ low 32 bits of the counter
            assert_eq!(priv_params, [0, 0, 0, 7, 5, 6, 7, 8]);
            let decrypted =
                decrypt(&ciphertext, PrivProtocol::Des, &key, 7, 1234, &priv_params).unwrap();
            assert_eq!(decrypted[..21], scoped_pdu);
        }
    }

    #[test]
    fn usm_aes_round_trip() {
        for auth in AuthProtocol::ALL {
            let key = localized_key("privpass12", &[1, 2, 3, 4, 5], auth);
            let scoped_pdu: Vec<u8> = (0..37).collect();
            let (ciphertext, priv_params) = encrypt(
                &scoped_pdu,
                PrivProtocol::Aes,
                &key,
                7,
                1234,
                0x0102_0304_0506_0708,
            );
            assert_eq!(ciphertext.len(), 37);
            assert_eq!(priv_params, [1, 2, 3, 4, 5, 6, 7, 8]);
            let decrypted = decrypt(&ciphertext, PrivProtocol::Aes, &key, 7, 1234, &priv_params);
            assert_eq!(decrypted, Ok(scoped_pdu));
        }
    }

    /// AES IV = engineBoots ‖ engineTime ‖ salt (RFC 3826 §3.1.2.1): the composition is checked with a NIST vector.
    #[test]
    fn usm_aes_iv_is_boots_time_salt() {
        let key = hex("2b7e151628aed2a6abf7158809cf4f3c");
        let plaintext = hex("6bc1bee22e409f96e93d7e117393172a");
        let (ciphertext, priv_params) = encrypt(
            &plaintext,
            PrivProtocol::Aes,
            &key,
            0x0001_0203,
            0x0405_0607,
            0x0809_0a0b_0c0d_0e0f,
        );
        assert_eq!(priv_params, hex("08090a0b0c0d0e0f")[..]);
        assert_eq!(ciphertext, hex("3b3fd92eb72dad20333449f8e83cfb4a"));
    }

    /// DES IV = pre-IV (last 8 bytes of the key) XOR salt (RFC 3414 §8.1.1.1): checked with a FIPS 81 vector.
    #[test]
    fn usm_des_iv_is_pre_iv_xor_salt() {
        // Key 0123456789abcdef; the pre-IV is all zeros, so IV = salt = 12345678 90abcdef.
        let key = hex("0123456789abcdef 0000000000000000");
        let (ciphertext, priv_params) = encrypt(
            b"Now is the time for all ",
            PrivProtocol::Des,
            &key,
            0x1234_5678,
            0,
            0x90ab_cdef,
        );
        assert_eq!(priv_params, hex("1234567890abcdef")[..]);
        assert_eq!(
            ciphertext,
            hex("e5c7cdde872bf27c 43e934008c389c0f 683788499a7c05f6")
        );
    }

    #[test]
    fn des_rejects_ciphertext_not_a_multiple_of_block() {
        let key = [1; 16];
        assert_eq!(
            decrypt(&[1, 2, 3], PrivProtocol::Des, &key, 0, 0, &[0; 8]),
            Err(SnmpError::Authentication(
                UsmFailure::InvalidPrivacyParameters
            ))
        );
    }

    #[test]
    fn rejects_wrong_priv_params_length() {
        let key = [1; 16];
        assert_eq!(
            decrypt(&[0; 8], PrivProtocol::Aes, &key, 0, 0, &[1, 2]),
            Err(SnmpError::Authentication(
                UsmFailure::InvalidPrivacyParameters
            ))
        );
    }
}
