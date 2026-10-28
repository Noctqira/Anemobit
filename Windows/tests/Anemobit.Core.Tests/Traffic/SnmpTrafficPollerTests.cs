using Anemobit.Core.Snmp;
using Anemobit.Core.Tests.Fakes;
using Anemobit.Core.Tests.Snmp.V3;
using Anemobit.Core.Traffic;
using Anemobit.Core.Transport;

namespace Anemobit.Core.Tests.Traffic;

public class SnmpTrafficPollerTests
{
    private static CancellationToken Token => TestContext.Current.CancellationToken;

    [Fact]
    public async Task RunAsync_CounterSequence_EmitsRatesPerInterval()
    {
        var recorder = new TransportRecorder(_ => Scripted(7,
        [
            (1_000_000, 100_000, 100),
            (2_250_000, 225_000, 200), // +1.25 MB / +125 kB → 10 Mb/s / 1 Mb/s
            (2_250_000, 225_000, 300), // Idle → 0
            (2_500_000, 237_500, 400), // 2 Mb/s / 100 kb/s
        ]));
        var clock = new FakeTimeProvider();

        var samples = await Samples(Poller(recorder, clock), "pppoe-out1", 3);

        Assert.Equal([10_000_000.0, 0, 2_000_000], samples.Select(s => s.InBps));
        Assert.Equal([1_000_000.0, 0, 100_000], samples.Select(s => s.OutBps));
        // sysUpTime (hundredths of a second) comes along too
        Assert.Equal([TimeSpan.FromSeconds(2), TimeSpan.FromSeconds(3), TimeSpan.FromSeconds(4)], samples.Select(s => s.DeviceUptime));
        Assert.True(samples.Zip(samples.Skip(1)).All(pair => pair.First.Timestamp <= pair.Second.Timestamp));

        // One GET per round, querying the resolved index 7
        var get = recorder.Transports[0].Sent.First(m => m.Pdu.Type == PduType.Get);
        Assert.Equal(
            [SnmpOids.IfHCInOctets.Appending(7), SnmpOids.IfHCOutOctets.Appending(7), SnmpOids.SysUpTime],
            get.Pdu.VarBinds.Select(b => b.Oid));
        // Polling interval is 1 second
        Assert.All(clock.Sleeps.Take(3), sleep => Assert.Equal(TimeSpan.FromSeconds(1), sleep));
    }

    [Fact]
    public async Task RunAsync_CounterReset_DoesNotProduceSpike()
    {
        var recorder = new TransportRecorder(_ => Scripted(7,
        [
            (9_000_000_000, 9_000_000_000, 1_000_000),
            (9_000_125_000, 9_000_000_000, 1_000_100), // 1 Mb/s
            (500, 500, 50),                            // Restarted → discarded
            (125_500, 500, 150),                       // 1 Mb/s
        ]));

        var samples = await Samples(Poller(recorder, new FakeTimeProvider()), "pppoe-out1", 2);

        Assert.Equal([1_000_000.0, 1_000_000], samples.Select(s => s.InBps));
    }

    [Fact]
    public async Task RunAsync_Startup_ReportsConnectingThenConnected()
    {
        var recorder = new TransportRecorder(_ => Scripted(7, [(0, 0, 1)]));

        var statuses = await Statuses(Poller(recorder, new FakeTimeProvider()), 2);

        Assert.Equal([new PollerStatus.Connecting(), new PollerStatus.Connected()], statuses);
    }

    [Fact]
    public async Task RunAsync_ConnectionFails_ReportsFailureAndRecovers()
    {
        var recorder = new TransportRecorder(attempt =>
        {
            if (attempt == 0)
            {
                var broken = new FakeTransport();
                broken.FailSends(new InvalidOperationException("unreachable"));
                return broken;
            }
            return Scripted(7, [(0, 0, 1)]);
        });
        var clock = new FakeTimeProvider();

        var statuses = await Statuses(Poller(recorder, clock), until: s => s is PollerStatus.Connected);

        Assert.Equal(4, statuses.Count);
        Assert.IsType<PollerStatus.Connecting>(statuses[0]);
        var failed = Assert.IsType<PollerStatus.Failed>(statuses[1]);
        Assert.Equal(TimeSpan.FromSeconds(1), failed.RetryIn);
        Assert.Equal(SnmpError.Transport, Assert.IsType<SnmpException>(failed.Error).Error);
        Assert.IsType<PollerStatus.Connecting>(statuses[2]);
        Assert.IsType<PollerStatus.Connected>(statuses[3]);
        Assert.Equal(TimeSpan.FromSeconds(1), clock.Sleeps[0]);
        // The failed connection must be closed
        Assert.True(recorder.Transports[0].DisposeCount >= 1);
    }

    [Fact]
    public async Task RunAsync_RepeatedFailures_BackOffUpToTenSeconds()
    {
        var recorder = new TransportRecorder(_ =>
        {
            var broken = new FakeTransport();
            broken.FailSends(new InvalidOperationException("unreachable"));
            return broken;
        });

        var statuses = await Statuses(Poller(recorder, new FakeTimeProvider()), until: s => s is PollerStatus.Failed, count: 6);

        Assert.Equal([1.0, 2, 4, 8, 10, 10], statuses.OfType<PollerStatus.Failed>().Select(f => f.RetryIn.TotalSeconds));
    }

    [Fact]
    public async Task RunAsync_TransportCannotBeCreated_ReportsFailure()
    {
        var poller = new SnmpTrafficPoller(
            Configuration(["pppoe-out1"]),
            (_, _, _) => throw new InvalidOperationException("cannot resolve"),
            new FakeTimeProvider());

        var failed = await FirstFailure(poller);

        Assert.IsType<InvalidOperationException>(failed.Error);
    }

    [Fact]
    public async Task RunAsync_MissingInterface_ReportsInterfaceNotFound()
    {
        var recorder = new TransportRecorder(_ => FakeTransport.ForAgent(new FakeAgent(FakeAgent.RouterOSInterfaceMib())));

        var failed = await FirstFailure(Poller(recorder, new FakeTimeProvider(), ["pppoe-out9"]));

        Assert.Equal("pppoe-out9", Assert.IsType<SnmpException>(failed.Error).InterfaceName);
    }

    [Fact]
    public async Task RunAsync_MissingOneOfSeveralInterfaces_ReportsThatInterface()
    {
        var recorder = new TransportRecorder(_ => FakeTransport.ForAgent(new FakeAgent(FakeAgent.RouterOSInterfaceMib())));

        var failed = await FirstFailure(Poller(recorder, new FakeTimeProvider(), ["pppoe-out1", "pppoe-out2"]));

        Assert.Equal("pppoe-out2", Assert.IsType<SnmpException>(failed.Error).InterfaceName);
    }

    [Fact]
    public async Task RunAsync_NoResponse_ReportsTimeout()
    {
        var recorder = new TransportRecorder(_ => new FakeTransport());

        var failed = await FirstFailure(Poller(recorder, new FakeTimeProvider()));

        Assert.Equal(SnmpError.Timeout, Assert.IsType<SnmpException>(failed.Error).Error);
    }

    [Fact]
    public async Task RunAsync_CountersDisappear_ReportsUnexpectedResponseAndReconnects()
    {
        // Interface index changed (e.g. renumbered after a device restart): the counter becomes noSuchInstance, so reconnect and resolve again.
        var agent = new FakeAgent(FakeAgent.RouterOSInterfaceMib());
        var recorder = new TransportRecorder(_ => FakeTransport.ForAgent(agent));
        var updates = new List<PollerUpdate>();

        await foreach (var update in Poller(recorder, new FakeTimeProvider()).RunAsync(Token))
        {
            updates.Add(update);
            if (update is PollerUpdate.StatusChanged { Status: PollerStatus.Connected } && recorder.Transports.Count == 1)
            {
                agent.Remove(SnmpOids.IfHCInOctets.Appending(7));
            }
            if (update is PollerUpdate.StatusChanged { Status: PollerStatus.Failed })
            {
                break;
            }
        }

        var failed = updates.OfType<PollerUpdate.StatusChanged>().Select(u => u.Status).OfType<PollerStatus.Failed>().Single();
        Assert.Equal(SnmpError.UnexpectedResponse, Assert.IsType<SnmpException>(failed.Error).Error);
    }

    [Fact]
    public async Task RunAsync_ConsumerStops_DisposesTransport()
    {
        var recorder = new TransportRecorder(_ => Scripted(7, [(0, 0, 1), (125, 0, 2)]));

        await Samples(Poller(recorder, new FakeTimeProvider()), "pppoe-out1", 1);

        Assert.All(recorder.Transports, t => Assert.True(t.DisposeCount >= 1));
    }

    [Fact]
    public async Task RunAsync_ConsumerCancels_EndsAndDisposesTransport()
    {
        var recorder = new TransportRecorder(_ => Scripted(7, [(0, 0, 1), (125, 0, 2)]));
        using var cts = CancellationTokenSource.CreateLinkedTokenSource(Token);

        await Assert.ThrowsAnyAsync<OperationCanceledException>(async () =>
        {
            await foreach (var update in Poller(recorder, new FakeTimeProvider()).RunAsync(cts.Token))
            {
                if (update is PollerUpdate.SnapshotReceived)
                {
                    await cts.CancelAsync();
                }
            }
        });

        Assert.All(recorder.Transports, t => Assert.True(t.DisposeCount >= 1));
    }

    [Fact]
    public async Task RunAsync_SeveralInterfaces_ReadsAllInOneGetAndComputesSeparately()
    {
        // pppoe-out1 = ifIndex 7, ether1 = ifIndex 1
        var recorder = new TransportRecorder(_ => Scripted(
        [
            (100, new() { [7] = (1_000_000, 100_000), [1] = (0, 0) }),
            (200, new() { [7] = (2_250_000, 225_000), [1] = (125_000, 250_000) }), // 10 / 1 Mb/s; 1 / 2 Mb/s
        ]));

        var snapshot = (await Snapshots(Poller(recorder, new FakeTimeProvider(), ["pppoe-out1", "ether1"]), 1))[0];

        Assert.Equal(10_000_000, snapshot.Samples["pppoe-out1"].InBps);
        Assert.Equal(1_000_000, snapshot.Samples["pppoe-out1"].OutBps);
        Assert.Equal(1_000_000, snapshot.Samples["ether1"].InBps);
        Assert.Equal(2_000_000, snapshot.Samples["ether1"].OutBps);
        Assert.Equal(TimeSpan.FromSeconds(2), snapshot.DeviceUptime);
        Assert.All(snapshot.Samples.Values, s => Assert.Equal(snapshot.Timestamp, s.Timestamp));

        // One GET reads all interfaces, in settings order, followed by sysUpTime
        var get = recorder.Transports[0].Sent.First(m => m.Pdu.Type == PduType.Get);
        Assert.Equal(
            [
                SnmpOids.IfHCInOctets.Appending(7), SnmpOids.IfHCOutOctets.Appending(7),
                SnmpOids.IfHCInOctets.Appending(1), SnmpOids.IfHCOutOctets.Appending(1),
                SnmpOids.SysUpTime,
            ],
            get.Pdu.VarBinds.Select(b => b.Oid));
    }

    [Fact]
    public async Task RunAsync_CounterResetOnOneInterface_KeepsTheOthers()
    {
        var recorder = new TransportRecorder(_ => Scripted(
        [
            (100, new() { [7] = (1_000_000, 0), [1] = (1_000_000, 0) }),
            (200, new() { [7] = (1_125_000, 0), [1] = (1_125_000, 0) }), // Both are 1 Mb/s
            (300, new() { [7] = (500, 0), [1] = (1_250_000, 0) }),       // ifIndex 7 counter reset → only it is discarded
            (400, new() { [7] = (125_500, 0), [1] = (1_375_000, 0) }),
        ]));

        var snapshots = await Snapshots(Poller(recorder, new FakeTimeProvider(), ["pppoe-out1", "ether1"]), 3);

        Assert.Equal([1_000_000.0, 1_000_000, 1_000_000], snapshots.Select(s => s.Samples["ether1"].InBps));
        Assert.Equal(
            [1_000_000.0, null, 1_000_000],
            snapshots.Select(s => s.Samples.TryGetValue("pppoe-out1", out var sample) ? sample.InBps : (double?)null));
    }

    [Fact]
    public async Task RunAsync_OverSnmpV3_ConnectsAndEmitsSamples()
    {
        var credentials = SnmpClientV3Tests.Credentials(SnmpAuthProtocol.Sha1, SnmpPrivProtocol.Aes);
        var agent = new FakeV3Agent(credentials);
        var poller = new SnmpTrafficPoller(
            Configuration(["pppoe-out1"]) with { Security = new SnmpSecurity.Usm(credentials) },
            (_, _, _) => Task.FromResult<IUdpTransport>(agent.CreateTransport()),
            new FakeTimeProvider());

        var updates = new List<PollerUpdate>();
        await foreach (var update in poller.RunAsync(Token))
        {
            updates.Add(update);
            if (update is PollerUpdate.SnapshotReceived)
            {
                break;
            }
        }

        Assert.Equal(
            [new PollerStatus.Connecting(), new PollerStatus.Connected()],
            updates.OfType<PollerUpdate.StatusChanged>().Select(u => u.Status));
        Assert.Equal(0, updates.OfType<PollerUpdate.SnapshotReceived>().Single().Snapshot.Samples["pppoe-out1"].InBps);
    }

    [Fact]
    public void Configuration_DuplicateInterfaceNames_KeepsFirstOccurrence()
    {
        var configuration = Configuration(["ether1", "pppoe-out1", "ether1"]);
        Assert.Equal(["ether1", "pppoe-out1"], configuration.InterfaceNames);
    }

    [Fact]
    public void Configuration_Defaults_MatchSpecification()
    {
        var configuration = new PollerConfiguration { Host = "192.0.2.1", Security = new SnmpSecurity.Community("public"), InterfaceNames = ["ether1"] };
        Assert.Equal(161, configuration.Port);
        Assert.Equal(TimeSpan.FromSeconds(1), configuration.Interval);
        Assert.Equal(SnmpClient.DefaultTimeout, configuration.Timeout);
        Assert.Equal(SnmpClient.DefaultRetries, configuration.Retries);
    }

    private static PollerConfiguration Configuration(IReadOnlyList<string> names) => new()
    {
        Host = "192.0.2.1",
        Security = new SnmpSecurity.Community("public"),
        InterfaceNames = names,
        Timeout = TimeSpan.FromMilliseconds(20),
        Retries = 0,
    };

    private static SnmpTrafficPoller Poller(TransportRecorder recorder, FakeTimeProvider clock, IReadOnlyList<string>? names = null) =>
        new(Configuration(names ?? ["pppoe-out1"]), recorder.Create, clock);

    /// <summary>Returns a fixed counter sequence by GET count; stays on the last entry once the sequence runs out.</summary>
    private static FakeTransport Scripted(uint ifIndex, (ulong In, ulong Out, uint UpTime)[] sequence) =>
        Scripted([.. sequence.Select(s => (s.UpTime, new Dictionary<uint, (ulong, ulong)> { [ifIndex] = (s.In, s.Out) }))]);

    /// <summary>Multi-interface version: each step gives sysUpTime and the counters for each ifIndex.</summary>
    private static FakeTransport Scripted((uint UpTime, Dictionary<uint, (ulong In, ulong Out)> Counters)[] steps)
    {
        var agent = new FakeAgent(FakeAgent.RouterOSInterfaceMib());
        var gets = 0;
        return FakeTransport.Responding((request, _) =>
        {
            if (request.Pdu.Type == PduType.Get)
            {
                var (upTime, counters) = steps[Math.Min(gets++, steps.Length - 1)];
                var values = new Dictionary<Oid, SnmpValue> { [SnmpOids.SysUpTime] = new SnmpValue.TimeTicks(upTime) };
                foreach (var (index, (inOctets, outOctets)) in counters)
                {
                    values[SnmpOids.IfHCInOctets.Appending(index)] = new SnmpValue.Counter64(inOctets);
                    values[SnmpOids.IfHCOutOctets.Appending(index)] = new SnmpValue.Counter64(outOctets);
                }
                agent.Update(values);
            }
            return [agent.Respond(request).Encode()];
        });
    }

    private static async Task<List<TrafficSnapshot>> Snapshots(SnmpTrafficPoller poller, int count)
    {
        var snapshots = new List<TrafficSnapshot>();
        await foreach (var update in poller.RunAsync(Token))
        {
            if (update is PollerUpdate.SnapshotReceived received)
            {
                snapshots.Add(received.Snapshot);
                if (snapshots.Count == count)
                {
                    break;
                }
            }
        }
        return snapshots;
    }

    private static async Task<List<TrafficSample>> Samples(SnmpTrafficPoller poller, string name, int count) =>
        [.. (await Snapshots(poller, count)).Select(s => s.Samples[name])];

    private static async Task<List<PollerStatus>> Statuses(
        SnmpTrafficPoller poller, int? total = null, Func<PollerStatus, bool>? until = null, int count = 1)
    {
        var statuses = new List<PollerStatus>();
        var matches = 0;
        await foreach (var update in poller.RunAsync(Token))
        {
            if (update is not PollerUpdate.StatusChanged changed)
            {
                continue;
            }
            statuses.Add(changed.Status);
            if (until?.Invoke(changed.Status) == true)
            {
                matches++;
            }
            if (statuses.Count == total || matches == count && until is not null)
            {
                break;
            }
        }
        return statuses;
    }

    private static async Task<PollerStatus.Failed> FirstFailure(SnmpTrafficPoller poller) =>
        (PollerStatus.Failed)(await Statuses(poller, until: s => s is PollerStatus.Failed)).Last();

    /// <summary>Records every transport the factory creates.</summary>
    private sealed class TransportRecorder(Func<int, FakeTransport> make)
    {
        private readonly Lock _lock = new();
        private readonly List<FakeTransport> _transports = [];

        public IReadOnlyList<FakeTransport> Transports
        {
            get
            {
                lock (_lock)
                {
                    return [.. _transports];
                }
            }
        }

        public Task<IUdpTransport> Create(string host, int port, CancellationToken cancellationToken)
        {
            lock (_lock)
            {
                var transport = make(_transports.Count);
                _transports.Add(transport);
                return Task.FromResult<IUdpTransport>(transport);
            }
        }
    }
}
