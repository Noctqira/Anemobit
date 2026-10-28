using System.Net;
using System.Net.Sockets;
using System.Threading.Channels;
using Anemobit.Core.Snmp;

namespace Anemobit.Core.Transport;

/// <summary>UDP transport built on <see cref="UdpClient"/>, connected to a single device (a connected socket that only receives packets from that address).</summary>
public sealed class UdpSocketTransport : IUdpTransport
{
    private readonly UdpClient _udp;
    private readonly CancellationTokenSource _closing = new();
    private readonly Channel<byte[]> _incoming = Channel.CreateBounded<byte[]>(
        new BoundedChannelOptions(64) { FullMode = BoundedChannelFullMode.DropOldest, SingleReader = true });
    private readonly Task _receiveLoop;
    private int _disposed;

    private UdpSocketTransport(UdpClient udp)
    {
        _udp = udp;
        _receiveLoop = Task.Run(ReceiveLoopAsync);
    }

    public ChannelReader<byte[]> Incoming => _incoming.Reader;

    /// <summary>Resolves the host name (IP addresses are used directly) and creates a socket connected to <paramref name="host"/>:<paramref name="port"/>.</summary>
    /// <exception cref="SnmpException">Name resolution or socket creation failed (<see cref="SnmpError.Transport"/>).</exception>
    public static async Task<UdpSocketTransport> ConnectAsync(string host, int port, CancellationToken cancellationToken = default)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(host);
        ArgumentOutOfRangeException.ThrowIfLessThan(port, IPEndPoint.MinPort);
        ArgumentOutOfRangeException.ThrowIfGreaterThan(port, IPEndPoint.MaxPort);

        if (!IPAddress.TryParse(host, out var address))
        {
            IPAddress[] addresses;
            try
            {
                addresses = await Dns.GetHostAddressesAsync(host, cancellationToken).ConfigureAwait(false);
            }
            catch (SocketException error)
            {
                throw SnmpException.Transport($"cannot resolve {host}: {error.Message}", error);
            }
            address = addresses.FirstOrDefault() ?? throw SnmpException.Transport($"cannot resolve {host}");
        }

        var udp = new UdpClient(address.AddressFamily);
        try
        {
            udp.Connect(new IPEndPoint(address, port));
            return new UdpSocketTransport(udp);
        }
        catch (SocketException error)
        {
            udp.Dispose();
            throw SnmpException.Transport(error.Message, error);
        }
    }

    public async ValueTask SendAsync(byte[] datagram, CancellationToken cancellationToken = default)
    {
        await _udp.SendAsync(datagram, cancellationToken).ConfigureAwait(false);
    }

    public async ValueTask DisposeAsync()
    {
        if (Interlocked.Exchange(ref _disposed, 1) != 0)
        {
            return;
        }
        await _closing.CancelAsync().ConfigureAwait(false);
        _udp.Dispose();
        await _receiveLoop.ConfigureAwait(false);
        _closing.Dispose();
    }

    private async Task ReceiveLoopAsync()
    {
        try
        {
            while (true)
            {
                UdpReceiveResult result;
                try
                {
                    result = await _udp.ReceiveAsync(_closing.Token).ConfigureAwait(false);
                }
                catch (SocketException error) when (error.SocketErrorCode == SocketError.ConnectionReset)
                {
                    // On Windows, an ICMP port unreachable makes the next receive fail; treat it as packet loss and leave it to the timeout and retries.
                    continue;
                }
                _incoming.Writer.TryWrite(result.Buffer);
            }
        }
        catch (Exception) when (_closing.IsCancellationRequested)
        {
            _incoming.Writer.TryComplete();
        }
        catch (Exception error)
        {
            _incoming.Writer.TryComplete(error);
        }
    }
}
