using System.Threading.Channels;
using Anemobit.Core.Snmp;
using Anemobit.Core.Transport;

namespace Anemobit.Core.Tests.Fakes;

/// <summary>Test transport: each send calls the responder to decide which datagrams to return. Never touches the network.</summary>
internal sealed class FakeTransport : IUdpTransport
{
    private readonly Channel<byte[]> _incoming = Channel.CreateUnbounded<byte[]>();
    private readonly Lock _lock = new();
    private readonly List<byte[]> _sentRaw = [];
    private RawResponder _responder;
    private int _disposeCount;
    private Exception? _sendError;

    public FakeTransport(RawResponder rawResponder)
    {
        _responder = rawResponder;
    }

    /// <summary>Never answers any request.</summary>
    public FakeTransport()
        : this((_, _) => [])
    {
    }

    public delegate IEnumerable<byte[]> RawResponder(byte[] datagram, int sendCount);

    public delegate IEnumerable<byte[]> Responder(SnmpMessage request, int sendCount);

    public ChannelReader<byte[]> Incoming => _incoming.Reader;

    /// <summary>Sent v2c messages (use <see cref="SentRaw"/> for v3 packets).</summary>
    public IReadOnlyList<SnmpMessage> Sent => [.. SentRaw.Select(SnmpMessage.Decode)];

    public IReadOnlyList<byte[]> SentRaw
    {
        get
        {
            lock (_lock)
            {
                return [.. _sentRaw];
            }
        }
    }

    public int DisposeCount
    {
        get
        {
            lock (_lock)
            {
                return _disposeCount;
            }
        }
    }

    /// <summary>Responds per v2c message (packets that fail to decode get no reply).</summary>
    public static FakeTransport Responding(Responder responder) => new((datagram, count) =>
    {
        SnmpMessage request;
        try
        {
            request = SnmpMessage.Decode(datagram);
        }
        catch (SnmpException)
        {
            return [];
        }
        return responder(request, count);
    });

    /// <summary><see cref="FakeAgent"/> answers every request.</summary>
    public static FakeTransport ForAgent(FakeAgent agent) =>
        Responding((request, _) => [agent.Respond(request).Encode()]);

    public void SetResponder(RawResponder responder)
    {
        lock (_lock)
        {
            _responder = responder;
        }
    }

    public void FailSends(Exception error)
    {
        lock (_lock)
        {
            _sendError = error;
        }
    }

    /// <summary>Simulates a remote connection failure: <see cref="Incoming"/> finishes with an error.</summary>
    public void FailIncoming(Exception error) => _incoming.Writer.TryComplete(error);

    /// <summary>Injects a received packet directly.</summary>
    public void Deliver(byte[] datagram) => _incoming.Writer.TryWrite(datagram);

    public ValueTask SendAsync(byte[] datagram, CancellationToken cancellationToken = default)
    {
        RawResponder responder;
        int count;
        Exception? error;
        lock (_lock)
        {
            _sentRaw.Add(datagram);
            (responder, count, error) = (_responder, _sentRaw.Count, _sendError);
        }
        if (error is not null)
        {
            return ValueTask.FromException(error);
        }

        foreach (var reply in responder(datagram, count))
        {
            _incoming.Writer.TryWrite(reply);
        }
        return ValueTask.CompletedTask;
    }

    public ValueTask DisposeAsync()
    {
        lock (_lock)
        {
            _disposeCount++;
        }
        _incoming.Writer.TryComplete();
        return ValueTask.CompletedTask;
    }
}
