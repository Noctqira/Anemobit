using System.Threading.Channels;

namespace Anemobit.Core.Transport;

/// <summary>
/// Transport layer the SNMP client uses to send and receive datagrams; unit tests replace it with a fake and never hit the real network.
/// <see cref="IAsyncDisposable.DisposeAsync"/> closes the connection and finishes <see cref="Incoming"/>; safe to call more than once.
/// </summary>
public interface IUdpTransport : IAsyncDisposable
{
    /// <summary>Received datagrams. Finishes normally when the transport closes, and with an error when the connection fails. Only one reader allowed.</summary>
    ChannelReader<byte[]> Incoming { get; }

    ValueTask SendAsync(byte[] datagram, CancellationToken cancellationToken = default);
}
