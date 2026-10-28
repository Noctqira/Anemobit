using System.Runtime.CompilerServices;
using System.Threading.Channels;
using Anemobit.Core.Formatting;
using Anemobit.Core.Snmp;
using Anemobit.Core.Transport;

namespace Anemobit.Core.Traffic;

/// <summary>
/// Poller that reads ifHCInOctets / ifHCOutOctets over SNMP (v2c or v3).
/// All interfaces' counters and sysUpTime are read in one GET, and each interface's rate is calculated separately.
/// </summary>
/// <param name="configuration">Connection and polling settings.</param>
/// <param name="transportFactory">Creates the transport; defaults to <see cref="UdpSocketTransport.ConnectAsync"/>.</param>
/// <param name="timeProvider">Clock for sample intervals and rate calculation; defaults to the system clock (monotonic).</param>
public sealed class SnmpTrafficPoller(
    PollerConfiguration configuration,
    UdpTransportFactory? transportFactory = null,
    TimeProvider? timeProvider = null) : ITrafficPoller
{
    public static readonly TimeSpan InitialBackoff = TimeSpan.FromSeconds(1);

    public static readonly TimeSpan MaximumBackoff = TimeSpan.FromSeconds(10);

    private readonly UdpTransportFactory _makeTransport = transportFactory
        ?? (async (host, port, cancellationToken) =>
            await UdpSocketTransport.ConnectAsync(host, port, cancellationToken).ConfigureAwait(false));

    private readonly TimeProvider _time = timeProvider ?? TimeProvider.System;

    public PollerConfiguration Configuration { get; } = configuration ?? throw new ArgumentNullException(nameof(configuration));

    public async IAsyncEnumerable<PollerUpdate> RunAsync([EnumeratorCancellation] CancellationToken cancellationToken = default)
    {
        var channel = Channel.CreateBounded<PollerUpdate>(new BoundedChannelOptions(64)
        {
            FullMode = BoundedChannelFullMode.DropOldest,
            SingleReader = true,
            SingleWriter = true,
        });
        using var stop = CancellationTokenSource.CreateLinkedTokenSource(cancellationToken);
        var producer = Task.Run(() => ProduceAsync(channel.Writer, stop.Token), CancellationToken.None);
        try
        {
            await foreach (var update in channel.Reader.ReadAllAsync(cancellationToken).ConfigureAwait(false))
            {
                yield return update;
            }
        }
        finally
        {
            // When the consumer stops (break, cancellation or exception), stop polling and wait for the connection to close.
            await stop.CancelAsync().ConfigureAwait(false);
            await producer.ConfigureAwait(false);
        }
    }

    private async Task ProduceAsync(ChannelWriter<PollerUpdate> writer, CancellationToken cancellationToken)
    {
        var backoff = new Backoff(InitialBackoff, MaximumBackoff);
        try
        {
            while (!cancellationToken.IsCancellationRequested)
            {
                writer.TryWrite(new PollerUpdate.StatusChanged(new PollerStatus.Connecting()));

                Exception failure;
                try
                {
                    await PollSessionAsync(writer, backoff, cancellationToken).ConfigureAwait(false);
                    return; // Only finishes normally when cancelled
                }
                catch (OperationCanceledException) when (cancellationToken.IsCancellationRequested)
                {
                    return;
                }
                catch (Exception error)
                {
                    failure = error;
                }

                var delay = backoff.Next();
                writer.TryWrite(new PollerUpdate.StatusChanged(new PollerStatus.Failed(failure, delay)));
                await Task.Delay(delay, _time, cancellationToken).ConfigureAwait(false);
            }
        }
        catch (OperationCanceledException) when (cancellationToken.IsCancellationRequested)
        {
            // Stop polling
        }
        finally
        {
            writer.TryComplete();
        }
    }

    /// <summary>Connects and resolves the interfaces, then polls until an error or cancellation.</summary>
    private async Task PollSessionAsync(ChannelWriter<PollerUpdate> writer, Backoff backoff, CancellationToken cancellationToken)
    {
        var transport = await _makeTransport(Configuration.Host, Configuration.Port, cancellationToken).ConfigureAwait(false);
        var client = new SnmpClient(transport, Configuration.Security, Configuration.Timeout, Configuration.Retries);
        await using (client.ConfigureAwait(false))
        {
            var names = Configuration.InterfaceNames;
            var indexes = await client.ResolveInterfaceIndexesAsync(names, cancellationToken).ConfigureAwait(false);
            // Two OIDs per interface (in, out), then sysUpTime.
            Oid[] oids =
            [
                .. indexes.SelectMany(index => new[] { SnmpOids.IfHCInOctets.Appending(index), SnmpOids.IfHCOutOctets.Appending(index) }),
                SnmpOids.SysUpTime,
            ];

            var origin = _time.GetTimestamp();
            var calculators = names.ToDictionary(name => name, _ => new RateCalculator(), StringComparer.Ordinal);
            var connected = false;
            var deadline = TimeSpan.Zero;

            while (true)
            {
                cancellationToken.ThrowIfCancellationRequested();
                var binds = await client.GetAsync(oids, cancellationToken).ConfigureAwait(false);
                var readings = Parse(binds, oids, names, _time.GetElapsedTime(origin));

                if (!connected)
                {
                    connected = true;
                    backoff.Reset();
                    writer.TryWrite(new PollerUpdate.StatusChanged(new PollerStatus.Connected()));
                }

                var timestamp = _time.GetUtcNow();
                var uptime = readings[0].Reading.SysUpTime is { } ticks ? Uptime.FromTimeTicks(ticks) : (TimeSpan?)null;
                var samples = new Dictionary<string, TrafficSample>(StringComparer.Ordinal);
                foreach (var (name, reading) in readings)
                {
                    if (calculators[name].Add(reading) is { } rate)
                    {
                        samples[name] = new TrafficSample(timestamp, rate.InBps, rate.OutBps, uptime);
                    }
                }
                if (samples.Count > 0)
                {
                    writer.TryWrite(new PollerUpdate.SnapshotReceived(new TrafficSnapshot(timestamp, uptime, samples)));
                }

                // Schedule on a fixed beat; if behind, don't catch up, just start from now.
                deadline += Configuration.Interval;
                var current = _time.GetElapsedTime(origin);
                if (deadline < current)
                {
                    deadline = current;
                }
                await Task.Delay(deadline - current, _time, cancellationToken).ConfigureAwait(false);
            }
        }
    }

    private static List<(string Name, CounterReading Reading)> Parse(
        IReadOnlyList<VarBind> binds, Oid[] oids, IReadOnlyList<string> names, TimeSpan timestamp)
    {
        if (binds.Count != oids.Length || !binds.Select(b => b.Oid).SequenceEqual(oids))
        {
            throw SnmpException.UnexpectedResponse("varbind count or OID mismatch");
        }
        uint? upTime = binds[^1].Value is SnmpValue.TimeTicks ticks ? ticks.Value : null;

        var readings = new List<(string, CounterReading)>(names.Count);
        for (var i = 0; i < names.Count; i++)
        {
            var inBind = binds[i * 2];
            var outBind = binds[(i * 2) + 1];
            if (inBind.Value.UnsignedValue is not { } inOctets || outBind.Value.UnsignedValue is not { } outOctets)
            {
                // Interface index changed (e.g. renumbered after a device restart): throw so the outer loop reconnects and resolves again.
                throw SnmpException.UnexpectedResponse($"interface counters unavailable for {names[i]} ({inBind.Value})");
            }
            readings.Add((names[i], new CounterReading(inOctets, outOctets, upTime, timestamp)));
        }
        return readings;
    }
}
