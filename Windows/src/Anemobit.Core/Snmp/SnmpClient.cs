using System.Diagnostics;
using System.Text;
using Anemobit.Core.Snmp.V3;
using Anemobit.Core.Transport;

namespace Anemobit.Core.Snmp;

/// <summary>
/// Minimal SNMP client: v2c community or SNMPv3 USM; request-id / msgID matching, timeouts and retries. Several requests can be in flight at once.
/// The client owns the transport: <see cref="DisposeAsync"/> also closes the transport.
/// </summary>
public sealed partial class SnmpClient : IAsyncDisposable
{
    public const int DefaultRetries = 1;

    public static readonly TimeSpan DefaultTimeout = TimeSpan.FromSeconds(2);

    private readonly IUdpTransport _transport;
    private readonly byte[] _community = [];
    private readonly UsmSession? _usm;
    private readonly TimeSpan _timeout;
    private readonly int _retries;
    private readonly long _clockOrigin = Stopwatch.GetTimestamp();

    private readonly Lock _lock = new();
    private readonly Dictionary<int, TaskCompletionSource<SnmpIncoming>> _pending = [];
    private Task? _receiveTask;
    private SnmpException? _terminalError;
    private int _nextRequestId = Random.Shared.Next(1, int.MaxValue / 2);
    private bool _disposed;

    public SnmpClient(IUdpTransport transport, SnmpSecurity security, TimeSpan? timeout = null, int retries = DefaultRetries)
    {
        ArgumentNullException.ThrowIfNull(transport);
        ArgumentNullException.ThrowIfNull(security);
        _transport = transport;
        _timeout = timeout ?? DefaultTimeout;
        _retries = Math.Max(0, retries);
        switch (security)
        {
            case SnmpSecurity.Community community:
                _community = Encoding.UTF8.GetBytes(community.Name);
                break;
            case SnmpSecurity.Usm usm:
                _usm = new UsmSession(usm.Credentials, Now);
                break;
            default:
                break;
        }
    }

    public SnmpClient(IUdpTransport transport, string community, TimeSpan? timeout = null, int retries = DefaultRetries)
        : this(transport, new SnmpSecurity.Community(community), timeout, retries)
    {
    }

    /// <summary>Local monotonic clock (0 when the client is created), used to estimate the SNMPv3 engineTime.</summary>
    private TimeSpan Now => Stopwatch.GetElapsedTime(_clockOrigin);

    public async Task<IReadOnlyList<VarBind>> GetAsync(IEnumerable<Oid> oids, CancellationToken cancellationToken = default)
    {
        var response = await RequestAsync(id => Pdu.Get(id, oids), cancellationToken).ConfigureAwait(false);
        return response.VarBinds;
    }

    public async Task<IReadOnlyList<VarBind>> GetNextAsync(IEnumerable<Oid> oids, CancellationToken cancellationToken = default)
    {
        var response = await RequestAsync(id => Pdu.GetNext(id, oids), cancellationToken).ConfigureAwait(false);
        return response.VarBinds;
    }

    public async Task<IReadOnlyList<VarBind>> GetBulkAsync(
        IEnumerable<Oid> oids, int maxRepetitions, int nonRepeaters = 0, CancellationToken cancellationToken = default)
    {
        var response = await RequestAsync(
            id => Pdu.GetBulk(id, nonRepeaters, maxRepetitions, oids), cancellationToken).ConfigureAwait(false);
        return response.VarBinds;
    }

    /// <summary>Closes the transport; all pending requests finish with <see cref="SnmpError.Transport"/>. Safe to call more than once.</summary>
    public async ValueTask DisposeAsync()
    {
        Task? receiveTask;
        lock (_lock)
        {
            if (_disposed)
            {
                return;
            }
            _disposed = true;
            receiveTask = _receiveTask;
        }

        Fail(SnmpException.Transport("closed"));
        await _transport.DisposeAsync().ConfigureAwait(false);
        if (receiveTask is not null)
        {
            await receiveTask.ConfigureAwait(false);
        }
    }

    private async Task<Pdu> RequestAsync(Func<int, Pdu> makePdu, CancellationToken cancellationToken)
    {
        lock (_lock)
        {
            if (_terminalError is not null)
            {
                throw _terminalError;
            }
        }
        StartReceivingIfNeeded();

        if (_usm is null)
        {
            var pdu = makePdu(MakeRequestId());
            var datagram = new SnmpMessage(_community, pdu).Encode();
            return Checked(await ExchangeAsync(pdu.RequestId, datagram, cancellationToken).ConfigureAwait(false));
        }
        return await RequestUsmAsync(_usm, makePdu, cancellationToken).ConfigureAwait(false);
    }

    private async Task<Pdu> RequestUsmAsync(UsmSession usm, Func<int, Pdu> makePdu, CancellationToken cancellationToken)
    {
        if (usm.Credentials.ValidationFailure is { } problem)
        {
            throw SnmpException.Authentication(problem);
        }
        await DiscoverIfNeededAsync(usm, cancellationToken).ConfigureAwait(false);

        // If the first attempt reports out-of-sync time, update the time and resend once.
        for (var attempt = 0; attempt < 2; attempt++)
        {
            var pdu = makePdu(MakeRequestId());
            byte[] datagram;
            lock (_lock)
            {
                datagram = usm.Encode(pdu, pdu.RequestId, Now);
            }

            SnmpIncoming incoming;
            try
            {
                incoming = await ExchangeAsync(pdu.RequestId, datagram, cancellationToken).ConfigureAwait(false);
            }
            catch (SnmpException error) when (error.Error == SnmpError.Timeout && usm.Credentials.Privacy is not null)
            {
                // Discovery succeeded (the device responds) but the encrypted request got no reply: usually the agent failed to decrypt and dropped the packet.
                throw SnmpException.Authentication(UsmFailure.NoResponseToEncryptedRequest, error);
            }

            if (incoming is SnmpIncoming.Report report && report.Oid == UsmReport.NotInTimeWindows && attempt == 0)
            {
                lock (_lock)
                {
                    usm.UpdateTime(report.Security, Now);
                }
                continue;
            }
            return Checked(incoming);
        }
        throw UsmReport.ErrorFor(UsmReport.NotInTimeWindows);
    }

    /// <summary>SNMPv3 engine discovery (RFC 3414 §4).</summary>
    private async Task DiscoverIfNeededAsync(UsmSession usm, CancellationToken cancellationToken)
    {
        lock (_lock)
        {
            if (usm.IsDiscovered)
            {
                return;
            }
        }
        var id = MakeRequestId();
        var incoming = await ExchangeAsync(id, UsmSession.DiscoveryMessage(id), cancellationToken).ConfigureAwait(false);
        if (incoming is not SnmpIncoming.Report report)
        {
            throw SnmpException.UnexpectedResponse("discovery got a response instead of a report");
        }
        lock (_lock)
        {
            usm.CompleteDiscovery(report.Security, Now);
        }
    }

    private static Pdu Checked(SnmpIncoming incoming) => incoming switch
    {
        SnmpIncoming.Response { Pdu.ErrorStatus: not (long)SnmpErrorStatus.NoError } response =>
            throw SnmpException.FromErrorStatus(response.Pdu.ErrorStatus, response.Pdu.ErrorIndex),
        SnmpIncoming.Response response => response.Pdu,
        SnmpIncoming.Report report => throw UsmReport.ErrorFor(report.Oid),
        _ => throw SnmpException.UnexpectedResponse("unknown message"),
    };

    /// <summary>Sends a packet and waits for the reply with the matching id; resends on timeout (same id), and throws Timeout when retries run out.</summary>
    private async Task<SnmpIncoming> ExchangeAsync(int id, byte[] datagram, CancellationToken cancellationToken)
    {
        var response = new TaskCompletionSource<SnmpIncoming>(TaskCreationOptions.RunContinuationsAsynchronously);
        lock (_lock)
        {
            if (_terminalError is not null)
            {
                throw _terminalError;
            }
            _pending[id] = response;
        }

        try
        {
            for (var attempt = 0; attempt <= _retries; attempt++)
            {
                await SendAsync(datagram, cancellationToken).ConfigureAwait(false);

                using var wait = CancellationTokenSource.CreateLinkedTokenSource(cancellationToken);
                var delay = Task.Delay(_timeout, wait.Token);
                var finished = await Task.WhenAny(response.Task, delay).ConfigureAwait(false);
                await wait.CancelAsync().ConfigureAwait(false);
                if (finished == response.Task)
                {
                    return await response.Task.ConfigureAwait(false);
                }
                cancellationToken.ThrowIfCancellationRequested();
            }
            throw SnmpException.Timeout();
        }
        finally
        {
            lock (_lock)
            {
                _pending.Remove(id);
            }
        }
    }

    private async Task SendAsync(byte[] datagram, CancellationToken cancellationToken)
    {
        try
        {
            await _transport.SendAsync(datagram, cancellationToken).ConfigureAwait(false);
        }
        catch (OperationCanceledException) when (cancellationToken.IsCancellationRequested)
        {
            throw;
        }
        catch (Exception error) when (error is not SnmpException)
        {
            throw SnmpException.Transport(error.Message, error);
        }
    }

    private int MakeRequestId()
    {
        lock (_lock)
        {
            var id = _nextRequestId;
            _nextRequestId = _nextRequestId == int.MaxValue ? 1 : _nextRequestId + 1;
            return id;
        }
    }

    private void StartReceivingIfNeeded()
    {
        lock (_lock)
        {
            if (_receiveTask is null && !_disposed)
            {
                _receiveTask = Task.Run(ReceiveLoopAsync);
            }
        }
    }

    private async Task ReceiveLoopAsync()
    {
        try
        {
            await foreach (var datagram in _transport.Incoming.ReadAllAsync().ConfigureAwait(false))
            {
                Handle(datagram);
            }
            Fail(SnmpException.Transport("connection closed"));
        }
        catch (Exception error)
        {
            Fail(SnmpException.Transport(error.Message, error));
        }
    }

    /// <summary>Ignores packets that are malformed, fail authentication, aren't a response / report, or have a mismatched id.</summary>
    private void Handle(byte[] datagram)
    {
        (int Id, SnmpIncoming Incoming)? decoded;
        TaskCompletionSource<SnmpIncoming>? waiting;
        lock (_lock)
        {
            decoded = _usm is not null ? _usm.Decode(datagram, Now) : DecodeV2c(datagram);
            if (decoded is null)
            {
                return;
            }
            _pending.TryGetValue(decoded.Value.Id, out waiting);
        }
        waiting?.TrySetResult(decoded.Value.Incoming);
    }

    private static (int Id, SnmpIncoming Incoming)? DecodeV2c(byte[] datagram)
    {
        try
        {
            var message = SnmpMessage.Decode(datagram);
            return message.Pdu.Type == PduType.Response
                ? (message.Pdu.RequestId, new SnmpIncoming.Response(message.Pdu))
                : null;
        }
        catch (SnmpException)
        {
            return null;
        }
    }

    private void Fail(SnmpException error)
    {
        TaskCompletionSource<SnmpIncoming>[] waiting;
        lock (_lock)
        {
            _terminalError ??= error;
            waiting = [.. _pending.Values];
        }
        foreach (var request in waiting)
        {
            request.TrySetException(error);
        }
    }
}
