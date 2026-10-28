using Anemobit.Core.Snmp;
using Anemobit.Core.Tests.Fakes;

namespace Anemobit.Core.Tests.Snmp;

public class SnmpClientWalkTests
{
    private static readonly TimeSpan ShortTimeout = SnmpClientTests.ShortTimeout;

    private static CancellationToken Token => TestContext.Current.CancellationToken;

    [Fact]
    public async Task WalkAsync_SubtreeLargerThanMaxRepetitions_UsesMultipleBulkRequests()
    {
        var transport = FakeTransport.ForAgent(RouterOS());
        await using var client = new SnmpClient(transport, "public", ShortTimeout);

        var binds = await client.WalkAsync(SnmpOids.IfName, maxRepetitions: 2, Token);

        Assert.Equal(new uint[] { 1, 2, 3, 7, 12 }.Select(SnmpOids.IfName.Appending), binds.Select(b => b.Oid));
        Assert.Equal(["ether1", "ether2", "bridge", "pppoe-out1", "wlan1"], binds.Select(b => b.Value.StringValue));
        Assert.Equal(3, transport.Sent.Count);
        Assert.All(transport.Sent, m =>
        {
            Assert.Equal(PduType.GetBulk, m.Pdu.Type);
            Assert.Equal(2, m.Pdu.ErrorIndex);
        });
    }

    [Fact]
    public async Task WalkAsync_EndOfMibView_StopsWalking()
    {
        var agent = new FakeAgent(new Dictionary<Oid, SnmpValue> { [SnmpOids.IfName.Appending(1)] = FakeAgent.Octets("ether1") });
        await using var client = new SnmpClient(FakeTransport.ForAgent(agent), "public", ShortTimeout);

        var binds = await client.WalkAsync(SnmpOids.IfName, cancellationToken: Token);

        Assert.Single(binds);
    }

    [Fact]
    public async Task WalkAsync_EmptySubtree_ReturnsEmpty()
    {
        var agent = new FakeAgent(new Dictionary<Oid, SnmpValue> { [SnmpOids.SysUpTime] = new SnmpValue.TimeTicks(1) });
        await using var client = new SnmpClient(FakeTransport.ForAgent(agent), "public", ShortTimeout);

        Assert.Empty(await client.WalkAsync(SnmpOids.IfName, cancellationToken: Token));
    }

    [Fact]
    public async Task WalkAsync_NonIncreasingOids_ThrowsUnexpectedResponse()
    {
        var looping = SnmpOids.IfName.Appending(1);
        var transport = FakeTransport.Responding((request, _) =>
            [request.Response([new VarBind(looping, new SnmpValue.OctetString([0x61]))])]);
        await using var client = new SnmpClient(transport, "public", ShortTimeout);

        var error = await Assert.ThrowsAsync<SnmpException>(() => client.WalkAsync(SnmpOids.IfName, cancellationToken: Token));
        Assert.Equal(SnmpError.UnexpectedResponse, error.Error);
    }

    [Fact]
    public async Task ListInterfacesAsync_RouterOS_ReturnsAllInterfaces()
    {
        await using var client = new SnmpClient(FakeTransport.ForAgent(RouterOS()), "public", ShortTimeout);

        var interfaces = await client.ListInterfacesAsync(Token);

        Assert.Equal([1u, 2, 3, 7, 12], interfaces.Select(i => i.Index));
        Assert.Equal("pppoe-out1", interfaces.Single(i => i.Index == 7).Name);
    }

    [Fact]
    public async Task ListInterfacesAsync_NoIfName_FallsBackToIfDescr()
    {
        var agent = new FakeAgent(new Dictionary<Oid, SnmpValue> { [SnmpOids.IfDescr.Appending(4)] = FakeAgent.Octets("eth0") });
        await using var client = new SnmpClient(FakeTransport.ForAgent(agent), "public", ShortTimeout);

        Assert.Equal([new SnmpInterface(4, "eth0")], await client.ListInterfacesAsync(Token));
    }

    [Fact]
    public async Task ResolveInterfaceIndexAsync_KnownName_ReturnsIndex()
    {
        await using var client = new SnmpClient(FakeTransport.ForAgent(RouterOS()), "public", ShortTimeout);

        Assert.Equal(7u, await client.ResolveInterfaceIndexAsync("pppoe-out1", Token));
    }

    [Fact]
    public async Task ResolveInterfaceIndexAsync_UnknownName_ThrowsInterfaceNotFound()
    {
        await using var client = new SnmpClient(FakeTransport.ForAgent(RouterOS()), "public", ShortTimeout);

        var error = await Assert.ThrowsAsync<SnmpException>(() => client.ResolveInterfaceIndexAsync("pppoe-out9", Token));
        Assert.Equal(SnmpError.InterfaceNotFound, error.Error);
        Assert.Equal("pppoe-out9", error.InterfaceName);
    }

    [Fact]
    public async Task ResolveInterfaceIndexesAsync_MultipleNames_KeepsRequestedOrder()
    {
        await using var client = new SnmpClient(FakeTransport.ForAgent(RouterOS()), "public", ShortTimeout);

        Assert.Equal([12u, 1, 7], await client.ResolveInterfaceIndexesAsync(["wlan1", "ether1", "pppoe-out1"], Token));
    }

    [Fact]
    public async Task ResolveInterfaceIndexesAsync_AnyNameMissing_ThrowsInterfaceNotFound()
    {
        await using var client = new SnmpClient(FakeTransport.ForAgent(RouterOS()), "public", ShortTimeout);

        var error = await Assert.ThrowsAsync<SnmpException>(() => client.ResolveInterfaceIndexesAsync(["ether1", "missing"], Token));
        Assert.Equal("missing", error.InterfaceName);
    }

    private static FakeAgent RouterOS() => new(FakeAgent.RouterOSInterfaceMib());
}
