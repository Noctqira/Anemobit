using System.Text;
using Anemobit.Core.Ber;

namespace Anemobit.Core.Snmp.V3;

/// <summary>
/// USM state for one SNMPv3 agent: engine info, localized keys, time sync, salt.
/// Not thread-safe; <see cref="SnmpClient"/> uses it inside a lock. Times are always passed in as local monotonic clock readings.
/// </summary>
internal sealed class UsmSession(UsmCredentials credentials, TimeSpan now)
{
    private uint _engineBoots;
    private uint _engineTimeAtReference;
    private TimeSpan _reference = now;
    private byte[] _authKey = [];
    private byte[] _privKey = [];
    private ulong _saltCounter = (ulong)Random.Shared.NextInt64();

    public UsmCredentials Credentials { get; } = credentials;

    public byte[]? EngineId { get; private set; }

    public bool IsDiscovered => EngineId is not null;

    private SnmpV3Flags Flags => Credentials.Privacy is null
        ? SnmpV3Flags.Reportable | SnmpV3Flags.Auth
        : SnmpV3Flags.Reportable | SnmpV3Flags.Auth | SnmpV3Flags.Priv;

    /// <summary>RFC 3414 §4: unauthenticated discovery request with an empty engineID.</summary>
    public static byte[] DiscoveryMessage(int msgId) => new SnmpV3Message(
        msgId,
        SnmpV3Flags.Reportable,
        new UsmSecurityParameters(),
        new SnmpV3Data.Plaintext(new ScopedPdu([], [], Pdu.Get(msgId)))).Encode().Bytes;

    /// <summary>Takes the engineID from the discovery response and localizes the keys.</summary>
    public void CompleteDiscovery(UsmSecurityParameters security, TimeSpan now)
    {
        if (security.EngineId.Length == 0)
        {
            throw SnmpException.UnexpectedResponse("no engine ID in discovery report");
        }
        EngineId = security.EngineId;
        var auth = Credentials.AuthProtocol;
        _authKey = UsmCrypto.LocalizedKey(Credentials.AuthPassword, security.EngineId, auth);
        if (Credentials.Privacy is { } privacy)
        {
            _privKey = UsmCrypto.LocalizedKey(privacy.Password, security.EngineId, auth);
        }
        UpdateTime(security, now);
    }

    public void UpdateTime(UsmSecurityParameters security, TimeSpan now)
    {
        _engineBoots = security.EngineBoots;
        _engineTimeAtReference = security.EngineTime;
        _reference = now;
    }

    /// <summary>Estimates the agent's current engineTime from locally elapsed time.</summary>
    public uint EngineTime(TimeSpan now)
    {
        var elapsed = Math.Max(0, (long)(now - _reference).TotalSeconds);
        return (uint)Math.Min(uint.MaxValue, _engineTimeAtReference + elapsed);
    }

    public byte[] Encode(Pdu pdu, int msgId, TimeSpan now)
    {
        if (EngineId is not { } engineId)
        {
            throw SnmpException.Authentication(UsmFailure.DiscoveryIncomplete);
        }

        var time = EngineTime(now);
        var scoped = new ScopedPdu(engineId, [], pdu);
        var security = new UsmSecurityParameters(
            engineId,
            _engineBoots,
            time,
            Encoding.UTF8.GetBytes(Credentials.UserName),
            new byte[Credentials.AuthProtocol.MacLength()]);

        SnmpV3Data data;
        if (Credentials.Privacy is { } privacy)
        {
            _saltCounter = unchecked(_saltCounter + 1);
            var (ciphertext, privParams) = UsmCrypto.Encrypt(scoped.Encode(), privacy.Protocol, _privKey, _engineBoots, time, _saltCounter);
            security = security with { PrivParams = privParams };
            data = new SnmpV3Data.Encrypted(ciphertext);
        }
        else
        {
            data = new SnmpV3Data.Plaintext(scoped);
        }

        var (bytes, range) = new SnmpV3Message(msgId, Flags, security, data).Encode();
        UsmCrypto.AuthenticationCode(_authKey, bytes, Credentials.AuthProtocol).CopyTo(bytes.AsSpan(range!.Value));
        return bytes;
    }

    /// <summary>Verifies and decrypts an incoming packet. Returns null when invalid (wrong HMAC, missing required authentication, etc.) so the caller ignores it.</summary>
    public (int MsgId, SnmpIncoming Incoming)? Decode(byte[] bytes, TimeSpan now)
    {
        SnmpV3Message message;
        Range authRange;
        try
        {
            (message, authRange) = SnmpV3Message.Decode(bytes);
        }
        catch (SnmpException)
        {
            return null;
        }

        var authenticated = message.Flags.HasFlag(SnmpV3Flags.Auth);
        if (authenticated && !HasValidMac(bytes, message, authRange))
        {
            return null;
        }

        var scoped = message.Data switch
        {
            SnmpV3Data.Plaintext plain => plain.ScopedPdu,
            SnmpV3Data.Encrypted encrypted when authenticated && message.Flags.HasFlag(SnmpV3Flags.Priv) =>
                TryDecrypt(encrypted.Ciphertext, message.Security),
            _ => null,
        };
        if (scoped is null)
        {
            return null;
        }

        switch (scoped.Pdu.Type)
        {
            case PduType.Report:
                if (authenticated)
                {
                    UpdateTime(message.Security, now);
                }
                var oid = scoped.Pdu.VarBinds.Count > 0 ? scoped.Pdu.VarBinds[0].Oid : null;
                return (message.MsgId, new SnmpIncoming.Report(oid, message.Security));
            case PduType.Response when authenticated:
                // When authentication is configured, unauthenticated responses are never accepted.
                UpdateTime(message.Security, now);
                return (message.MsgId, new SnmpIncoming.Response(scoped.Pdu));
            default:
                return null;
        }
    }

    private bool HasValidMac(byte[] bytes, SnmpV3Message message, Range authRange)
    {
        if (EngineId is not { } engineId
            || !message.Security.EngineId.AsSpan().SequenceEqual(engineId)
            || message.Security.AuthParams.Length != Credentials.AuthProtocol.MacLength())
        {
            return false;
        }
        var zeroed = (byte[])bytes.Clone();
        zeroed.AsSpan(authRange).Clear();
        var expected = UsmCrypto.AuthenticationCode(_authKey, zeroed, Credentials.AuthProtocol);
        return expected.AsSpan().SequenceEqual(message.Security.AuthParams);
    }

    private ScopedPdu? TryDecrypt(byte[] ciphertext, UsmSecurityParameters security)
    {
        if (Credentials.Privacy is not { } privacy)
        {
            return null;
        }
        try
        {
            var plaintext = UsmCrypto.Decrypt(ciphertext, privacy.Protocol, _privKey, security.EngineBoots, security.EngineTime, security.PrivParams);
            // DES-decrypted data may carry padding bytes; read only the first SEQUENCE.
            return ScopedPdu.Decode(new BerReader(plaintext));
        }
        catch (Exception error) when (error is SnmpException or BerException)
        {
            return null;
        }
    }
}
