using System.Runtime.CompilerServices;
using System.Threading.Channels;
using Anemobit.Core.Traffic;

namespace Anemobit.Tests.Fakes;

/// <summary>Poller whose events are sent by the test; records the configuration it was created with and whether it has stopped.</summary>
internal sealed class FakePoller(PollerConfiguration configuration) : ITrafficPoller
{
    private readonly Channel<PollerUpdate> _updates = Channel.CreateUnbounded<PollerUpdate>();

    public PollerConfiguration Configuration { get; } = configuration;

    public bool Stopped { get; private set; }

    public void Send(PollerStatus status) => _updates.Writer.TryWrite(new PollerUpdate.StatusChanged(status));

    public void Send(TrafficSnapshot snapshot) => _updates.Writer.TryWrite(new PollerUpdate.SnapshotReceived(snapshot));

    /// <summary>Ends enumeration (so the view model's consumer work completes and the test can wait for it).</summary>
    public void Complete() => _updates.Writer.TryComplete();

    public async IAsyncEnumerable<PollerUpdate> RunAsync([EnumeratorCancellation] CancellationToken cancellationToken = default)
    {
        try
        {
            await foreach (var update in _updates.Reader.ReadAllAsync(cancellationToken))
            {
                yield return update;
            }
        }
        finally
        {
            Stopped = true;
        }
    }
}
