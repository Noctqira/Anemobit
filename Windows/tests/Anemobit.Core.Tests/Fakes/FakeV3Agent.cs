using System.Text;
using Anemobit.Core.Ber;
using Anemobit.Core.Snmp;
using Anemobit.Core.Snmp.V3;

namespace Anemobit.Core.Tests.Fakes;

/// <summary>Simulated SNMPv3 agent (USM). MIB lookups go to <see cref="FakeAgent"/>; this handles discovery, authentication, privacy and the various Reports.</summary>
internal sealed class FakeV3Agent
{
    public const uint EngineBoots = 5;
    private const uint EngineTime = 10_000;

    private readonly Lock _lock = new();
    private readonly List<SnmpV3Message> _requests = [];
    private readonly byte[] _authKey;
    private readonly byte[] _privKey;
    private readonly UsmCredentials _credentials;
    private readonly FakeAgent _mib;

    public FakeV3Agent(UsmCredentials credentials, FakeAgent? mib = null)
    {
        _credentials = credentials;
        _mib = mib ?? new FakeAgent(FakeAgent.RouterOSInterfaceMib());
        _authKey = UsmCrypto.LocalizedKey(credentials.AuthPassword, EngineId, credentials.AuthProtocol);
        _privKey = credentials.Privacy is { } privacy
            ? UsmCrypto.LocalizedKey(privacy.Password, EngineId, credentials.AuthProtocol)
            : [];
    }

    public static byte[] EngineId { get; } = [0x80, 0x00, 0x3A, 0x8C, 0x04, 0x50, 0x47];

    /// <summary>Discovery responses carry the wrong time (like some agents), forcing the client to resend after notInTimeWindows.</summary>
    public bool DiscoveryOmitsTime { get; set; }

    public IReadOnlyList<SnmpV3Message> Requests
    {
        get
        {
            lock (_lock)
            {
                return [.. _requests];
            }
        }
    }

    private byte[] UserName => Encoding.UTF8.GetBytes(_credentials.UserName);

    /// <summary>Creates a transport whose requests are all answered by this agent.</summary>
    public FakeTransport CreateTransport() => new((datagram, _) => Respond(datagram));

    public IEnumerable<byte[]> Respond(byte[] bytes)
    {
        SnmpV3Message message;
        Range authRange;
        try
        {
            (message, authRange) = SnmpV3Message.Decode(bytes);
        }
        catch (SnmpException)
        {
            return [];
        }
        lock (_lock)
        {
            _requests.Add(message);
        }

        var security = message.Security;
        // Discovery
        if (security.EngineId.Length == 0)
        {
            return [Report(UsmReport.UnknownEngineIds, message.MsgId, DiscoveryOmitsTime ? 0 : EngineTime, authenticated: false)];
        }
        if (!security.UserName.AsSpan().SequenceEqual(UserName))
        {
            return [Report(UsmReport.UnknownUserNames, message.MsgId, EngineTime, authenticated: false)];
        }
        var wantsPriv = _credentials.Privacy is not null;
        if (!message.Flags.HasFlag(SnmpV3Flags.Auth) || message.Flags.HasFlag(SnmpV3Flags.Priv) != wantsPriv)
        {
            return [Report(UsmReport.UnsupportedSecLevels, message.MsgId, EngineTime, authenticated: false)];
        }

        var zeroed = (byte[])bytes.Clone();
        zeroed.AsSpan(authRange).Clear();
        if (!UsmCrypto.AuthenticationCode(_authKey, zeroed, _credentials.AuthProtocol).AsSpan().SequenceEqual(security.AuthParams))
        {
            return [Report(UsmReport.WrongDigests, message.MsgId, EngineTime, authenticated: false)];
        }
        if (security.EngineBoots != EngineBoots || Math.Abs((long)security.EngineTime - EngineTime) > 150)
        {
            return [Report(UsmReport.NotInTimeWindows, message.MsgId, EngineTime, authenticated: true)];
        }

        ScopedPdu scoped;
        switch (message.Data)
        {
            case SnmpV3Data.Plaintext plain:
                scoped = plain.ScopedPdu;
                break;
            case SnmpV3Data.Encrypted encrypted:
                var decoded = TryDecrypt(encrypted.Ciphertext, security);
                if (decoded is null)
                {
                    return [Report(UsmReport.DecryptionErrors, message.MsgId, EngineTime, authenticated: false)];
                }
                scoped = decoded;
                break;
            default:
                return [];
        }

        var response = _mib.Respond(new SnmpMessage([], scoped.Pdu)).Pdu;
        return [Encode(new ScopedPdu(EngineId, [], response), message.MsgId, EngineTime)];
    }

    /// <summary>Encodes an authenticated (and, when required, encrypted) response.</summary>
    public byte[] Encode(ScopedPdu scoped, int msgId, uint time)
    {
        var security = new UsmSecurityParameters(EngineId, EngineBoots, time, UserName, new byte[_credentials.AuthProtocol.MacLength()], []);
        var flags = SnmpV3Flags.Auth;
        SnmpV3Data data;
        if (_credentials.Privacy is { } privacy)
        {
            flags |= SnmpV3Flags.Priv;
            var (ciphertext, privParams) = UsmCrypto.Encrypt(scoped.Encode(), privacy.Protocol, _privKey, EngineBoots, time, (ulong)msgId);
            security = security with { PrivParams = privParams };
            data = new SnmpV3Data.Encrypted(ciphertext);
        }
        else
        {
            data = new SnmpV3Data.Plaintext(scoped);
        }
        return Sign(new SnmpV3Message(msgId, flags, security, data));
    }

    private ScopedPdu? TryDecrypt(byte[] ciphertext, UsmSecurityParameters security)
    {
        if (_credentials.Privacy is not { } privacy)
        {
            return null;
        }
        try
        {
            var plaintext = UsmCrypto.Decrypt(ciphertext, privacy.Protocol, _privKey, security.EngineBoots, security.EngineTime, security.PrivParams);
            return ScopedPdu.Decode(new BerReader(plaintext));
        }
        catch (Exception error) when (error is SnmpException or BerException)
        {
            return null;
        }
    }

    private byte[] Report(Oid oid, int msgId, uint time, bool authenticated)
    {
        var pdu = new Pdu(PduType.Report, msgId, 0, 0, [new VarBind(oid, new SnmpValue.Counter32(1))]);
        var scoped = new ScopedPdu(EngineId, [], pdu);
        if (authenticated)
        {
            // Authenticated Reports aren't encrypted (RFC 3414 allows sending Reports as authNoPriv).
            var security = new UsmSecurityParameters(EngineId, EngineBoots, time, UserName, new byte[_credentials.AuthProtocol.MacLength()], []);
            return Sign(new SnmpV3Message(msgId, SnmpV3Flags.Auth, security, new SnmpV3Data.Plaintext(scoped)));
        }
        return new SnmpV3Message(
            msgId,
            SnmpV3Flags.None,
            new UsmSecurityParameters(EngineId, EngineBoots, time),
            new SnmpV3Data.Plaintext(scoped)).Encode().Bytes;
    }

    private byte[] Sign(SnmpV3Message message)
    {
        var (bytes, range) = message.Encode();
        UsmCrypto.AuthenticationCode(_authKey, bytes, _credentials.AuthProtocol).CopyTo(bytes.AsSpan(range!.Value));
        return bytes;
    }
}
