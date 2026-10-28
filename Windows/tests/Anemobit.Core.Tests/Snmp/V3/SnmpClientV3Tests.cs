using Anemobit.Core.Snmp;
using Anemobit.Core.Snmp.V3;
using Anemobit.Core.Tests.Fakes;

namespace Anemobit.Core.Tests.Snmp.V3;

public class SnmpClientV3Tests
{
    private static readonly TimeSpan ShortTimeout = TimeSpan.FromMilliseconds(50);

    private static CancellationToken Token => TestContext.Current.CancellationToken;

    public static TheoryData<SnmpAuthProtocol, SnmpPrivProtocol?> SecurityCombinations => new()
    {
        { SnmpAuthProtocol.Md5, null },
        { SnmpAuthProtocol.Sha1, null },
        { SnmpAuthProtocol.Md5, SnmpPrivProtocol.Des },
        { SnmpAuthProtocol.Md5, SnmpPrivProtocol.Aes },
        { SnmpAuthProtocol.Sha1, SnmpPrivProtocol.Des },
        { SnmpAuthProtocol.Sha1, SnmpPrivProtocol.Aes },
        { SnmpAuthProtocol.Sha256, null },
        { SnmpAuthProtocol.Sha256, SnmpPrivProtocol.Aes },
        { SnmpAuthProtocol.Sha384, SnmpPrivProtocol.Des },
        { SnmpAuthProtocol.Sha512, SnmpPrivProtocol.Aes },
    };

    [Theory]
    [MemberData(nameof(SecurityCombinations))]
    public async Task GetAsync_AnySecurityCombination_DiscoversThenAuthenticates(SnmpAuthProtocol auth, SnmpPrivProtocol? priv)
    {
        var credentials = Credentials(auth, priv);
        var agent = new FakeV3Agent(credentials);
        await using var client = Client(agent.CreateTransport(), credentials);

        var binds = await client.GetAsync([SnmpOids.SysUpTime], Token);

        Assert.Equal([new VarBind(SnmpOids.SysUpTime, new SnmpValue.TimeTicks(100))], binds);
        var requests = agent.Requests;
        Assert.Equal(2, requests.Count);
        Assert.Equal(SnmpV3Flags.Reportable, requests[0].Flags);
        Assert.Empty(requests[0].Security.EngineId);
        Assert.True(requests[1].Flags.HasFlag(SnmpV3Flags.Auth));
        Assert.Equal(priv is not null, requests[1].Flags.HasFlag(SnmpV3Flags.Priv));
        Assert.Equal(FakeV3Agent.EngineId, requests[1].Security.EngineId);
        if (priv is not null)
        {
            Assert.IsType<SnmpV3Data.Encrypted>(requests[1].Data);
        }

        // The second request doesn't run discovery again
        await client.GetAsync([SnmpOids.SysUpTime], Token);
        Assert.Equal(3, agent.Requests.Count);
    }

    [Fact]
    public async Task ResolveInterfaceIndexAsync_OverV3_WalksInterfaces()
    {
        var credentials = Credentials(SnmpAuthProtocol.Sha1, SnmpPrivProtocol.Aes);
        await using var client = Client(new FakeV3Agent(credentials).CreateTransport(), credentials);

        Assert.Equal(7u, await client.ResolveInterfaceIndexAsync("pppoe-out1", Token));
    }

    [Fact]
    public async Task GetAsync_UnknownUser_ThrowsUnknownUserName()
    {
        var agent = new FakeV3Agent(Credentials(SnmpAuthProtocol.Sha1, SnmpPrivProtocol.Aes));
        await using var client = Client(agent.CreateTransport(), Credentials(SnmpAuthProtocol.Sha1, SnmpPrivProtocol.Aes, user: "nobody"));

        await AssertUsmFailureAsync(UsmFailure.UnknownUserName, client);
    }

    [Fact]
    public async Task GetAsync_WrongAuthPassword_ThrowsWrongDigest()
    {
        var agent = new FakeV3Agent(Credentials(SnmpAuthProtocol.Sha1, SnmpPrivProtocol.Aes));
        await using var client = Client(agent.CreateTransport(), Credentials(SnmpAuthProtocol.Sha1, SnmpPrivProtocol.Aes, authPassword: "wrongpass99"));

        await AssertUsmFailureAsync(UsmFailure.WrongDigest, client);
    }

    [Fact]
    public async Task GetAsync_WrongAuthProtocol_ThrowsWrongDigest()
    {
        var agent = new FakeV3Agent(Credentials(SnmpAuthProtocol.Sha1, null));
        await using var client = Client(agent.CreateTransport(), Credentials(SnmpAuthProtocol.Md5, null));

        await AssertUsmFailureAsync(UsmFailure.WrongDigest, client);
    }

    [Theory]
    [InlineData(SnmpPrivProtocol.Des)]
    [InlineData(SnmpPrivProtocol.Aes)]
    public async Task GetAsync_WrongPrivPassword_ThrowsDecryptionError(SnmpPrivProtocol priv)
    {
        var agent = new FakeV3Agent(Credentials(SnmpAuthProtocol.Sha1, priv));
        await using var client = Client(agent.CreateTransport(), Credentials(SnmpAuthProtocol.Sha1, priv, privPassword: "wrongpriv99"));

        await AssertUsmFailureAsync(UsmFailure.DecryptionError, client);
    }

    [Fact]
    public async Task GetAsync_EncryptedRequestSilentlyDropped_ThrowsNoResponseToEncryptedRequest()
    {
        // Real agents (net-snmp, RouterOS) often drop the packet without a Report when decryption fails.
        var credentials = Credentials(SnmpAuthProtocol.Sha1, SnmpPrivProtocol.Aes);
        var agent = new FakeV3Agent(credentials);
        await using var client = Client(new FakeTransport((bytes, _) => AnswerDiscoveryOnly(agent, bytes)), credentials);

        await AssertUsmFailureAsync(UsmFailure.NoResponseToEncryptedRequest, client);
    }

    [Fact]
    public async Task GetAsync_AuthNoPrivRequestUnanswered_ThrowsTimeout()
    {
        var credentials = Credentials(SnmpAuthProtocol.Sha1, null);
        var agent = new FakeV3Agent(credentials);
        await using var client = Client(new FakeTransport((bytes, _) => AnswerDiscoveryOnly(agent, bytes)), credentials);

        var error = await Assert.ThrowsAsync<SnmpException>(() => client.GetAsync([SnmpOids.SysUpTime], Token));
        Assert.Equal(SnmpError.Timeout, error.Error);
    }

    [Fact]
    public async Task GetAsync_SecurityLevelMismatch_ThrowsUnsupportedSecurityLevel()
    {
        var agent = new FakeV3Agent(Credentials(SnmpAuthProtocol.Sha1, SnmpPrivProtocol.Aes));
        await using var client = Client(agent.CreateTransport(), Credentials(SnmpAuthProtocol.Sha1, null));

        await AssertUsmFailureAsync(UsmFailure.UnsupportedSecurityLevel, client);
    }

    [Fact]
    public async Task GetAsync_NotInTimeWindow_UpdatesTimeAndRetriesOnce()
    {
        var credentials = Credentials(SnmpAuthProtocol.Md5, SnmpPrivProtocol.Des);
        var agent = new FakeV3Agent(credentials) { DiscoveryOmitsTime = true };
        await using var client = Client(agent.CreateTransport(), credentials);

        var binds = await client.GetAsync([SnmpOids.SysUpTime], Token);

        Assert.Equal(new SnmpValue.TimeTicks(100), binds[0].Value);
        // Discovery, rejection (time out of sync), then resend after updating the time
        Assert.Equal(3, agent.Requests.Count);
        Assert.True(agent.Requests[2].Security.EngineTime >= 10_000);
    }

    [Fact]
    public async Task GetAsync_ResponseWithBadMac_IsIgnored()
    {
        var credentials = Credentials(SnmpAuthProtocol.Sha1, null);
        var agent = new FakeV3Agent(credentials);
        var transport = new FakeTransport((bytes, _) => agent.Respond(bytes).Select(reply =>
        {
            var (message, range) = SnmpV3Message.Decode(reply);
            if (!message.Flags.HasFlag(SnmpV3Flags.Auth))
            {
                return reply; // Discovery responses aren't authenticated; pass them through unchanged
            }
            var tampered = (byte[])reply.Clone();
            tampered[range.Start] ^= 0xFF;
            return tampered;
        }).ToList());
        await using var client = Client(transport, credentials);

        var error = await Assert.ThrowsAsync<SnmpException>(() => client.GetAsync([SnmpOids.SysUpTime], Token));
        Assert.Equal(SnmpError.Timeout, error.Error);
    }

    [Theory]
    [InlineData("short", "privpass12", UsmFailure.AuthPasswordTooShort)]
    [InlineData("authpass12", "short", UsmFailure.PrivPasswordTooShort)]
    public async Task GetAsync_ShortPassword_ThrowsBeforeSending(string authPassword, string privPassword, UsmFailure expected)
    {
        var transport = new FakeTransport();
        await using var client = Client(transport, Credentials(SnmpAuthProtocol.Sha1, SnmpPrivProtocol.Aes, authPassword: authPassword, privPassword: privPassword));

        await AssertUsmFailureAsync(expected, client);
        Assert.Empty(transport.SentRaw);
    }

    [Fact]
    public void ToString_Credentials_DoesNotExposePasswords()
    {
        var credentials = Credentials(SnmpAuthProtocol.Sha1, SnmpPrivProtocol.Aes, authPassword: "auth-secret-1", privPassword: "priv-secret-2");
        var text = new SnmpSecurity.Usm(credentials).ToString();
        Assert.DoesNotContain("secret", text, StringComparison.Ordinal);
        Assert.DoesNotContain("secret", new SnmpSecurity.Community("community-secret").ToString(), StringComparison.Ordinal);
    }

    internal static UsmCredentials Credentials(
        SnmpAuthProtocol auth,
        SnmpPrivProtocol? priv,
        string user = "anemobit",
        string authPassword = "authpass12",
        string privPassword = "privpass12") =>
        new(user, auth, authPassword, priv is { } protocol ? new UsmPrivacy(protocol, privPassword) : null);

    private static SnmpClient Client(FakeTransport transport, UsmCredentials credentials) =>
        new(transport, new SnmpSecurity.Usm(credentials), ShortTimeout, retries: 0);

    private static IEnumerable<byte[]> AnswerDiscoveryOnly(FakeV3Agent agent, byte[] bytes) =>
        SnmpV3Message.Decode(bytes).Message.Security.EngineId.Length == 0 ? agent.Respond(bytes) : [];

    private static async Task AssertUsmFailureAsync(UsmFailure expected, SnmpClient client)
    {
        var error = await Assert.ThrowsAsync<SnmpException>(() => client.GetAsync([SnmpOids.SysUpTime], Token));
        Assert.Equal(SnmpError.Authentication, error.Error);
        Assert.Equal(expected, error.UsmFailure);
    }
}
