namespace Anemobit.Core.Transport;

/// <summary>Creates a transport connected to <paramref name="host"/>:<paramref name="port"/> (replaced with a fake in tests).</summary>
public delegate Task<IUdpTransport> UdpTransportFactory(string host, int port, CancellationToken cancellationToken);
