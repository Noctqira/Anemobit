using System.Collections.ObjectModel;
using System.Globalization;
using Anemobit.Core.Traffic;
using Anemobit.Infrastructure;
using Anemobit.Resources;
using Anemobit.Settings;
using Anemobit.Text;

namespace Anemobit.ViewModels;

/// <summary>
/// Drives the main window, floating window, notification area flyout and tooltip. Polling runs in the poller's background work; results are received here on the UI thread.
/// </summary>
public sealed class TrafficViewModel : ObservableObject, IDisposable
{
    private readonly SettingsStore _settings;
    private readonly Func<PollerConfiguration, ITrafficPoller> _makePoller;
    private readonly TimeProvider _time;
    private CancellationTokenSource? _running;
    private PollerConfiguration? _active;
    private ConnectionState _state = new ConnectionState.NotConfigured();
    private DateTimeOffset? _lastUpdated;
    private TimeSpan? _deviceUptime;

    public TrafficViewModel(SettingsStore settings, Func<PollerConfiguration, ITrafficPoller> makePoller, TimeProvider? timeProvider = null)
    {
        _settings = settings;
        _makePoller = makePoller;
        _time = timeProvider ?? TimeProvider.System;
        _settings.Changed += OnSettingsChanged;
    }

    /// <summary>The latest polling work; tests use it to wait for consumption to finish.</summary>
    public Task Completion { get; private set; } = Task.CompletedTask;

    public AppSettings Settings => _settings.Current;

    public ConnectionState State
    {
        get => _state;
        private set
        {
            if (SetProperty(ref _state, value))
            {
                OnPropertiesChanged(
                    nameof(IsConfigured), nameof(StatusKind), nameof(StatusText), nameof(StatusDetail),
                    nameof(ErrorMessage), nameof(FloatingText), nameof(FloatingDownText), nameof(FloatingUpText),
                    nameof(TrayToolTip), nameof(Title), nameof(Subtitle));
            }
        }
    }

    /// <summary>Monitored interfaces, in settings order.</summary>
    public ObservableCollection<InterfaceTraffic> Interfaces { get; } = [];

    /// <summary>Interface shown in the floating window and notification area flyout.</summary>
    public InterfaceTraffic? FloatingInterface =>
        Interfaces.FirstOrDefault(i => i.Name == Settings.EffectiveFloatingInterface) ?? Interfaces.FirstOrDefault();

    public bool IsConfigured => _state is not ConnectionState.NotConfigured;

    /// <summary>Charts are always shown for a single interface; with several, it follows the setting.</summary>
    public bool ShowsCharts => Settings.ChartsWhenMultiple || Interfaces.Count == 1;

    public DateTimeOffset? LastUpdated => _lastUpdated;

    public TimeSpan? DeviceUptime => _deviceUptime;

    public string DeviceUpText => _deviceUptime is { } uptime
        ? string.Format(CultureInfo.CurrentCulture, Strings.DeviceUpFormat, UptimeText.Format(uptime))
        : string.Empty;

    public string UpdatedText => _lastUpdated is { } updated
        ? string.Format(CultureInfo.CurrentCulture, Strings.UpdatedFormat, updated.ToLocalTime().ToString("T", CultureInfo.CurrentCulture))
        : string.Empty;

    public StatusKind StatusKind => _state switch
    {
        ConnectionState.Connected => StatusKind.Ok,
        ConnectionState.Connecting => StatusKind.Connecting,
        ConnectionState.Failed => StatusKind.Failed,
        _ => StatusKind.NotSet,
    };

    public string StatusText => _state switch
    {
        ConnectionState.Connected => "OK",
        ConnectionState.Connecting => Strings.StatusConnecting,
        ConnectionState.Failed failed => string.Format(CultureInfo.CurrentCulture, Strings.StatusErrorRetryFormat, (int)failed.RetryIn.TotalSeconds),
        _ => Strings.StatusNotSet,
    };

    /// <summary>Status pill tooltip: the error message on failure.</summary>
    public string StatusDetail => ErrorMessage ?? StatusText;

    public string? ErrorMessage => _state is ConnectionState.Failed failed ? ErrorText.Describe(failed.Error) : null;

    /// <summary>Floating window text.</summary>
    public string FloatingText => _state switch
    {
        ConnectionState.NotConfigured => Strings.TrayNotSet,
        ConnectionState.Connected => FloatingInterface?.Summary ?? "↓ … ↑ …",
        ConnectionState.Failed => "↓ -- ↑ --",
        _ => "↓ … ↑ …",
    };

    /// <summary>Floating window download text ("↓ 12.3 Mbps"; "↓ …" while connecting, "↓ --" on failure).</summary>
    public string FloatingDownText => "↓ " + FloatingValue(i => i.CurrentIn);

    /// <summary>Floating window upload text.</summary>
    public string FloatingUpText => "↑ " + FloatingValue(i => i.CurrentOut);

    /// <summary>Notification area icon tooltip (Windows limits it to 127 characters).</summary>
    public string TrayToolTip
    {
        get
        {
            var text = IsConfigured && FloatingInterface is { } floating
                ? $"Anemobit · {floating.Name}\n{FloatingText}"
                : Strings.TrayNotSet;
            return text.Length <= 127 ? text : text[..127];
        }
    }

    /// <summary>Main window title bar: the interface name for a single interface, the device address for several.</summary>
    public string Title => !IsConfigured ? "Anemobit" : Interfaces.Count == 1 ? Interfaces[0].Name : Settings.Host;

    public string Subtitle => !IsConfigured
        ? string.Empty
        : Interfaces.Count == 1
            ? Settings.Host
            : string.Format(CultureInfo.CurrentCulture, Strings.InterfaceCountFormat, Interfaces.Count);

    /// <summary>Starts polling with the current settings (clears the charts).</summary>
    public void Start()
    {
        Stop();
        _active = Settings.ToPollerConfiguration();
        Interfaces.Clear();
        foreach (var name in _active?.InterfaceNames ?? [])
        {
            Interfaces.Add(new InterfaceTraffic(name, Settings.UnitStyle, _time));
        }
        _lastUpdated = null;
        _deviceUptime = null;
        OnPropertiesChanged(nameof(LastUpdated), nameof(DeviceUptime), nameof(DeviceUpText), nameof(UpdatedText), nameof(ShowsCharts));
        NotifyFloatingChanged();

        if (_active is null)
        {
            State = new ConnectionState.NotConfigured();
            return;
        }
        State = new ConnectionState.Connecting();
        _running = new CancellationTokenSource();
        Completion = ConsumeAsync(_makePoller(_active), _running.Token);
    }

    /// <summary>Stops polling; the poller closes the connection when enumeration ends.</summary>
    public void Stop()
    {
        if (_running is null)
        {
            return;
        }
        _running.Cancel();
        _running.Dispose();
        _running = null;
    }

    public void Dispose()
    {
        _settings.Changed -= OnSettingsChanged;
        Stop();
    }

    /// <summary>Restarts polling only if connection settings (including the refresh rate) changed; changing only the display style doesn't interrupt.</summary>
    private void OnSettingsChanged(object? sender, EventArgs e)
    {
        if (!SameConnection(Settings.ToPollerConfiguration(), _active))
        {
            Start();
            return;
        }
        foreach (var traffic in Interfaces)
        {
            traffic.UnitStyle = Settings.UnitStyle;
        }
        OnPropertiesChanged(nameof(Settings), nameof(ShowsCharts));
        NotifyFloatingChanged();
    }

    private async Task ConsumeAsync(ITrafficPoller poller, CancellationToken cancellationToken)
    {
        try
        {
            await foreach (var update in poller.RunAsync(cancellationToken))
            {
                switch (update)
                {
                    case PollerUpdate.SnapshotReceived received:
                        Apply(received.Snapshot);
                        break;
                    case PollerUpdate.StatusChanged changed:
                        Apply(changed.Status);
                        break;
                    default:
                        break;
                }
            }
        }
        catch (OperationCanceledException) when (cancellationToken.IsCancellationRequested)
        {
            // Stopped or settings changed
        }
    }

    private void Apply(TrafficSnapshot snapshot)
    {
        foreach (var traffic in Interfaces)
        {
            if (snapshot.Samples.TryGetValue(traffic.Name, out var sample))
            {
                traffic.Append(sample);
            }
        }
        _lastUpdated = snapshot.Timestamp;
        _deviceUptime = snapshot.DeviceUptime;
        OnPropertiesChanged(nameof(LastUpdated), nameof(DeviceUptime), nameof(DeviceUpText), nameof(UpdatedText));
        NotifyFloatingChanged();
    }

    private void Apply(PollerStatus status)
    {
        State = status switch
        {
            PollerStatus.Connected => new ConnectionState.Connected(),
            PollerStatus.Failed failed => new ConnectionState.Failed(failed.Error, failed.RetryIn),
            _ => new ConnectionState.Connecting(),
        };
        if (status is PollerStatus.Failed)
        {
            foreach (var traffic in Interfaces)
            {
                traffic.ClearLatest();
            }
        }
        NotifyFloatingChanged();
    }

    private void NotifyFloatingChanged() => OnPropertiesChanged(
        nameof(FloatingInterface), nameof(FloatingText), nameof(FloatingDownText), nameof(FloatingUpText),
        nameof(TrayToolTip), nameof(Title), nameof(Subtitle));

    private string FloatingValue(Func<InterfaceTraffic, string> select) => _state switch
    {
        ConnectionState.Connected when FloatingInterface?.Latest is not null => select(FloatingInterface),
        ConnectionState.Failed => "--",
        _ => "…",
    };

    private static bool SameConnection(PollerConfiguration? a, PollerConfiguration? b) =>
        a is null || b is null
            ? a is null && b is null
            : a.Host == b.Host
              && a.Port == b.Port
              && a.Security == b.Security
              && a.Interval == b.Interval
              && a.Timeout == b.Timeout
              && a.Retries == b.Retries
              && a.InterfaceNames.SequenceEqual(b.InterfaceNames);
}
