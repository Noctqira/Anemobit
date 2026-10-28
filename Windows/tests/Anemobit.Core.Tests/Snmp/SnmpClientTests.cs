using Anemobit.Core.Snmp;
using Anemobit.Core.Tests.Fakes;

namespace Anemobit.Core.Tests.Snmp;

public class SnmpClientTests
{
    internal static readonly TimeSpan ShortTimeout = TimeSpan.FromMilliseconds(50);

    private static CancellationToken Token => TestContext.Current.CancellationToken;

    [Fact]
    public void Defaults_MatchSpecification()
    {
        Assert.Equal(TimeSpan.FromSeconds(2), SnmpClient.DefaultTimeout);
        Assert.Equal(1, SnmpClient.DefaultRetries);
    }

    [Fact]
    public async Task GetAsync_Response_ReturnsVarBinds()
    {
        var transport = FakeTransport.Responding((request, _) => [request.Response([UpTime(4242)])]);
        await using var client = new SnmpClient(transport, "public", ShortTimeout);

        var binds = await client.GetAsync([SnmpOids.SysUpTime], Token);

        Assert.Equal([UpTime(4242)], binds);
        var sent = Assert.Single(transport.Sent);
        Assert.Equal("public"u8.ToArray(), sent.Community);
        Assert.Equal(PduType.Get, sent.Pdu.Type);
    }

    [Fact]
    public async Task GetAsync_ResponseWithMismatchedRequestId_IsIgnored()
    {
        var transport = FakeTransport.Responding((request, _) =>
        [
            request.Response([UpTime(1)], requestId: unchecked(request.Pdu.RequestId + 1)),
            request.Response([UpTime(2)]),
        ]);
        await using var client = new SnmpClient(transport, "public", ShortTimeout);

        var binds = await client.GetAsync([SnmpOids.SysUpTime], Token);

        Assert.Equal(new SnmpValue.TimeTicks(2), binds[0].Value);
    }

    [Fact]
    public async Task GetAsync_OnlyMismatchedResponses_ThrowsTimeout()
    {
        var transport = FakeTransport.Responding((request, _) =>
            [request.Response([], requestId: unchecked(request.Pdu.RequestId + 1))]);
        await using var client = new SnmpClient(transport, "public", ShortTimeout, retries: 1);

        await AssertSnmpErrorAsync(SnmpError.Timeout, () => client.GetAsync([SnmpOids.SysUpTime], Token));
    }

    [Fact]
    public async Task GetAsync_GarbageAndNonResponsePdus_AreIgnored()
    {
        var transport = FakeTransport.Responding((request, _) =>
        [
            [0xDE, 0xAD],
            request.Encode(), // Echo of our own GetRequest
            request.Response([UpTime(9)]),
        ]);
        await using var client = new SnmpClient(transport, "public", ShortTimeout);

        var binds = await client.GetAsync([SnmpOids.SysUpTime], Token);

        Assert.Equal(new SnmpValue.TimeTicks(9), binds[0].Value);
    }

    [Fact]
    public async Task GetAsync_FirstPacketLost_RetriesWithSameRequestId()
    {
        var transport = FakeTransport.Responding((request, sendCount) =>
            sendCount >= 2 ? [request.Response([UpTime(7)])] : []);
        await using var client = new SnmpClient(transport, "public", ShortTimeout, retries: 1);

        var binds = await client.GetAsync([SnmpOids.SysUpTime], Token);

        Assert.Equal(new SnmpValue.TimeTicks(7), binds[0].Value);
        Assert.Equal(2, transport.Sent.Count);
        Assert.Equal(transport.Sent[0].Pdu.RequestId, transport.Sent[1].Pdu.RequestId);
    }

    [Fact]
    public async Task GetAsync_NoResponse_ThrowsTimeoutAfterRetries()
    {
        var transport = new FakeTransport();
        await using var client = new SnmpClient(transport, "public", ShortTimeout, retries: 1);

        await AssertSnmpErrorAsync(SnmpError.Timeout, () => client.GetAsync([SnmpOids.SysUpTime], Token));
        Assert.Equal(2, transport.Sent.Count);
    }

    [Fact]
    public async Task GetAsync_ResponseWithErrorStatus_ThrowsErrorStatus()
    {
        var transport = FakeTransport.Responding((request, _) =>
            [request.Response(request.Pdu.VarBinds, errorStatus: 2, errorIndex: 1)]);
        await using var client = new SnmpClient(transport, "public", ShortTimeout);

        var error = await AssertSnmpErrorAsync(SnmpError.ErrorStatus, () => client.GetAsync([SnmpOids.SysUpTime], Token));
        Assert.Equal(2, error.Status);
        Assert.Equal(1, error.ErrorIndex);
    }

    [Fact]
    public async Task GetAsync_MissingInstance_ReturnsNoSuchInstance()
    {
        var agent = new FakeAgent(new Dictionary<Oid, SnmpValue> { [SnmpOids.SysUpTime] = new SnmpValue.TimeTicks(1) });
        await using var client = new SnmpClient(FakeTransport.ForAgent(agent), "public", ShortTimeout);

        var binds = await client.GetAsync([SnmpOids.IfHCInOctets.Appending(99)], Token);

        Assert.Equal(new SnmpValue.NoSuchInstance(), binds[0].Value);
    }

    [Fact]
    public async Task GetAsync_ConsecutiveRequests_UseDistinctRequestIds()
    {
        var agent = new FakeAgent(new Dictionary<Oid, SnmpValue> { [SnmpOids.SysUpTime] = new SnmpValue.TimeTicks(1) });
        var transport = FakeTransport.ForAgent(agent);
        await using var client = new SnmpClient(transport, "public", ShortTimeout);

        await client.GetAsync([SnmpOids.SysUpTime], Token);
        await client.GetAsync([SnmpOids.SysUpTime], Token);

        Assert.NotEqual(transport.Sent[0].Pdu.RequestId, transport.Sent[1].Pdu.RequestId);
    }

    [Fact]
    public async Task GetAsync_ConcurrentRequestsAnsweredInReverse_AreMatchedById()
    {
        // Don't answer the first request yet; after the second is sent, answer both at once (in reverse order).
        var held = new List<SnmpMessage>();
        var transport = FakeTransport.Responding((request, sendCount) =>
        {
            lock (held)
            {
                held.Add(request);
                if (sendCount < 2)
                {
                    return [];
                }
                return [.. held.AsEnumerable().Reverse().Select(r => r.Response(
                    [new VarBind(r.Pdu.VarBinds[0].Oid, new SnmpValue.Counter64((ulong)r.Pdu.RequestId))]))];
            }
        });
        await using var client = new SnmpClient(transport, "public", TimeSpan.FromSeconds(5));

        var first = client.GetAsync([SnmpOids.IfHCInOctets.Appending(1)], Token);
        var second = client.GetAsync([SnmpOids.IfHCInOctets.Appending(2)], Token);
        var results = await Task.WhenAll(first, second);

        var sent = transport.Sent;
        Assert.Equal(new SnmpValue.Counter64((ulong)sent[0].Pdu.RequestId), results[0][0].Value);
        Assert.Equal(new SnmpValue.Counter64((ulong)sent[1].Pdu.RequestId), results[1][0].Value);
    }

    [Fact]
    public async Task GetAsync_SendFails_ThrowsTransport()
    {
        var transport = new FakeTransport();
        transport.FailSends(new InvalidOperationException("boom"));
        await using var client = new SnmpClient(transport, "public", ShortTimeout);

        await AssertSnmpErrorAsync(SnmpError.Transport, () => client.GetAsync([SnmpOids.SysUpTime], Token));
    }

    [Fact]
    public async Task GetAsync_IncomingFails_ThrowsTransport()
    {
        var transport = new FakeTransport();
        transport.SetResponder((_, _) =>
        {
            transport.FailIncoming(new InvalidOperationException("boom"));
            return [];
        });
        await using var client = new SnmpClient(transport, "public", TimeSpan.FromSeconds(5));

        await AssertSnmpErrorAsync(SnmpError.Transport, () => client.GetAsync([SnmpOids.SysUpTime], Token));
    }

    [Fact]
    public async Task GetAsync_AfterIncomingFailure_FailsWithoutSending()
    {
        var transport = new FakeTransport();
        transport.SetResponder((_, _) =>
        {
            transport.FailIncoming(new InvalidOperationException("boom"));
            return [];
        });
        await using var client = new SnmpClient(transport, "public", TimeSpan.FromSeconds(5));
        await Assert.ThrowsAsync<SnmpException>(() => client.GetAsync([SnmpOids.SysUpTime], Token));
        var sentBefore = transport.SentRaw.Count;

        await AssertSnmpErrorAsync(SnmpError.Transport, () => client.GetAsync([SnmpOids.SysUpTime], Token));
        Assert.Equal(sentBefore, transport.SentRaw.Count);
    }

    [Fact]
    public async Task GetAsync_Cancelled_ThrowsOperationCanceled()
    {
        await using var client = new SnmpClient(new FakeTransport(), "public", TimeSpan.FromSeconds(10));
        using var cts = CancellationTokenSource.CreateLinkedTokenSource(Token);

        var request = client.GetAsync([SnmpOids.SysUpTime], cts.Token);
        await Task.Delay(20, Token);
        await cts.CancelAsync();

        await Assert.ThrowsAnyAsync<OperationCanceledException>(() => request);
    }

    [Fact]
    public async Task DisposeAsync_Always_DisposesTransportOnce()
    {
        var transport = new FakeTransport();
        var client = new SnmpClient(transport, "public");

        await client.DisposeAsync();
        await client.DisposeAsync();

        Assert.Equal(1, transport.DisposeCount);
    }

    [Fact]
    public async Task DisposeAsync_PendingRequest_ThrowsTransport()
    {
        var client = new SnmpClient(new FakeTransport(), "public", TimeSpan.FromSeconds(10));
        var request = client.GetAsync([SnmpOids.SysUpTime], Token);
        await Task.Delay(20, Token);

        await client.DisposeAsync();

        await AssertSnmpErrorAsync(SnmpError.Transport, () => request);
    }

    private static VarBind UpTime(uint ticks) => new(SnmpOids.SysUpTime, new SnmpValue.TimeTicks(ticks));

    private static async Task<SnmpException> AssertSnmpErrorAsync(SnmpError expected, Func<Task> action)
    {
        var error = await Assert.ThrowsAsync<SnmpException>(action);
        Assert.Equal(expected, error.Error);
        return error;
    }
}
