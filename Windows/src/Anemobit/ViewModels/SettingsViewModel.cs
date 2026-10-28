using System.Collections.ObjectModel;
using System.Globalization;
using System.Text;
using Anemobit.Core.Formatting;
using Anemobit.Core.Snmp;
using Anemobit.Core.Transport;
using Anemobit.Infrastructure;
using Anemobit.Resources;
using Anemobit.Settings;
using Anemobit.Text;

namespace Anemobit.ViewModels;

/// <summary>Draft state for Settings. Only Save writes the settings and restarts polling; Open at Login and the display language apply immediately.</summary>
public sealed class SettingsViewModel : ObservableObject, IDisposable
{
    private readonly SettingsStore _store;
    private readonly UdpTransportFactory _makeTransport;
    private readonly LaunchAtLogin? _launchAtLogin;
    private readonly AppLanguage _launchLanguage;
    private readonly TimeSpan? _timeout;
    private readonly List<string> _selected;
    private List<SnmpInterface> _scanned = [];
    private CancellationTokenSource? _work;

    private string _host;
    private string _portText;
    private SecurityMode _mode;
    private string _community;
    private SnmpAuthProtocol _authProtocol;
    private string _authPassword;
    private SnmpPrivProtocol _privProtocol;
    private string _privPassword;
    private string _floatingInterface;
    private int _refreshSeconds;
    private RateUnitStyle _unitStyle;
    private bool _chartsWhenMultiple;
    private string? _downloadColor;
    private string? _uploadColor;
    private bool _showFloatingWindow;
    private bool _floatingAlwaysOnTop;
    private double _floatingOpacityPercent;
    private ActivityKind _activityKind;
    private string _activityText = Strings.ChangesTakeEffect;
    private string? _startupError;
    private AppLanguage _language;
    private string? _languageError;

    /// <param name="store">Settings.</param>
    /// <param name="makeTransport">Transport for scanning and testing the connection.</param>
    /// <param name="launchAtLogin">Open at Login; when null the toggle does nothing (tests).</param>
    /// <param name="launchLanguage">Display language this run started with; Restart Now appears when the choice differs.</param>
    /// <param name="restart">Restarts Anemobit; when null Restart Now does nothing (tests).</param>
    /// <param name="timeout">SNMP timeout for scanning and testing; defaults to <see cref="SnmpClient.DefaultTimeout"/>.</param>
    public SettingsViewModel(
        SettingsStore store,
        UdpTransportFactory makeTransport,
        LaunchAtLogin? launchAtLogin,
        AppLanguage launchLanguage = AppLanguage.System,
        Action? restart = null,
        TimeSpan? timeout = null)
    {
        _store = store;
        _makeTransport = makeTransport;
        _launchAtLogin = launchAtLogin;
        _launchLanguage = launchLanguage;
        _timeout = timeout;

        var settings = store.Current;
        _host = settings.Host;
        _portText = settings.Port.ToString(CultureInfo.InvariantCulture);
        _mode = settings.Security.Mode;
        _community = settings.Security.Community;
        _authProtocol = settings.Security.AuthProtocol;
        _authPassword = settings.Security.AuthPassword;
        _privProtocol = settings.Security.PrivProtocol;
        _privPassword = settings.Security.PrivPassword;
        _selected = [.. settings.InterfaceNames];
        _floatingInterface = settings.EffectiveFloatingInterface ?? string.Empty;
        _refreshSeconds = settings.RefreshSeconds;
        _unitStyle = settings.UnitStyle;
        _chartsWhenMultiple = settings.ChartsWhenMultiple;
        _downloadColor = settings.DownloadColor;
        _uploadColor = settings.UploadColor;
        _showFloatingWindow = settings.ShowFloatingWindow;
        _floatingAlwaysOnTop = settings.FloatingAlwaysOnTop;
        _floatingOpacityPercent = Math.Round(settings.FloatingOpacity * 100);
        _language = settings.Language;

        ScanCommand = new RelayCommand(() => _ = ScanAsync(), () => CanConnect && !IsWorking);
        TestCommand = new RelayCommand(() => _ = TestAsync(), () => CanConnect && !IsWorking);
        SaveCommand = new RelayCommand(Save, () => CanSave);
        ResetDownloadColorCommand = new RelayCommand(() => DownloadColor = null);
        ResetUploadColorCommand = new RelayCommand(() => UploadColor = null);
        RestartCommand = new RelayCommand(() => restart?.Invoke());
        RebuildChoices();
    }

    /// <summary>Raised after a successful save (the Settings window can close or update).</summary>
    public event EventHandler? Saved;

    public static IReadOnlyList<SecurityMode> SecurityModes { get; } = Enum.GetValues<SecurityMode>();

    public static IReadOnlyList<SnmpAuthProtocol> AuthProtocols { get; } = Enum.GetValues<SnmpAuthProtocol>();

    public static IReadOnlyList<SnmpPrivProtocol> PrivProtocols { get; } = Enum.GetValues<SnmpPrivProtocol>();

    public static IReadOnlyList<int> RefreshIntervals => AppSettings.RefreshIntervals;

    public static IReadOnlyList<RateUnitStyle> UnitStyles { get; } = Enum.GetValues<RateUnitStyle>();

    public static IReadOnlyList<AppLanguage> Languages { get; } = Enum.GetValues<AppLanguage>();

    public RelayCommand ScanCommand { get; }

    public RelayCommand TestCommand { get; }

    public RelayCommand SaveCommand { get; }

    public RelayCommand ResetDownloadColorCommand { get; }

    public RelayCommand ResetUploadColorCommand { get; }

    public RelayCommand RestartCommand { get; }

    // Device

    public string Host
    {
        get => _host;
        set => SetDraft(ref _host, InputFilter.Host(value));
    }

    public string PortText
    {
        get => _portText;
        set => SetDraft(ref _portText, InputFilter.Port(value));
    }

    public int? Port => int.TryParse(_portText, NumberStyles.None, CultureInfo.InvariantCulture, out var port) && port is > 0 and <= 65_535
        ? port
        : null;

    // SNMP security

    public SecurityMode Mode
    {
        get => _mode;
        set
        {
            if (SetDraft(ref _mode, value))
            {
                OnPropertiesChanged(nameof(IsCommunityMode), nameof(IsUsmMode), nameof(IsPrivateMode), nameof(ModeExplanation));
            }
        }
    }

    public bool IsCommunityMode => _mode == SecurityMode.None;

    public bool IsUsmMode => _mode != SecurityMode.None;

    public bool IsPrivateMode => _mode == SecurityMode.Private;

    public string ModeExplanation => _mode switch
    {
        SecurityMode.None => Strings.ModeNoneExplanation,
        SecurityMode.Authorized => Strings.ModeAuthExplanation,
        _ => Strings.ModePrivExplanation,
    };

    public string Community
    {
        get => _community;
        set => SetDraft(ref _community, value);
    }

    public SnmpAuthProtocol AuthProtocol
    {
        get => _authProtocol;
        set => SetDraft(ref _authProtocol, value);
    }

    public string AuthPassword
    {
        get => _authPassword;
        set => SetDraft(ref _authPassword, value);
    }

    public SnmpPrivProtocol PrivProtocol
    {
        get => _privProtocol;
        set => SetDraft(ref _privProtocol, value);
    }

    public string PrivPassword
    {
        get => _privPassword;
        set => SetDraft(ref _privPassword, value);
    }

    // Interfaces

    /// <summary>Interface list: scan results plus names that are selected but not in the results (e.g. before scanning).</summary>
    public ObservableCollection<InterfaceChoice> InterfaceChoices { get; } = [];

    public bool HasInterfaceChoices => InterfaceChoices.Count > 0;

    /// <summary>Selected interfaces, in list order.</summary>
    public IReadOnlyList<string> SelectedInterfaces => _selected;

    public bool HasMultipleSelected => _selected.Count > 1;

    public string FloatingInterface
    {
        get => _floatingInterface;
        set => SetProperty(ref _floatingInterface, value ?? string.Empty);
    }

    // Display

    public int RefreshSeconds
    {
        get => _refreshSeconds;
        set => SetProperty(ref _refreshSeconds, value);
    }

    public RateUnitStyle UnitStyle
    {
        get => _unitStyle;
        set => SetProperty(ref _unitStyle, value);
    }

    public bool ChartsWhenMultiple
    {
        get => _chartsWhenMultiple;
        set => SetProperty(ref _chartsWhenMultiple, value);
    }

    // Floating window

    public bool ShowFloatingWindow
    {
        get => _showFloatingWindow;
        set => SetProperty(ref _showFloatingWindow, value);
    }

    public bool FloatingAlwaysOnTop
    {
        get => _floatingAlwaysOnTop;
        set => SetProperty(ref _floatingAlwaysOnTop, value);
    }

    /// <summary>0–100 (%).</summary>
    public double FloatingOpacityPercent
    {
        get => _floatingOpacityPercent;
        set => SetProperty(ref _floatingOpacityPercent, Math.Clamp(Math.Round(value), 0, 100));
    }

    // Colors

    /// <summary>#RRGGBB; null means the theme default.</summary>
    public string? DownloadColor
    {
        get => _downloadColor;
        set
        {
            if (SetProperty(ref _downloadColor, HexColor.Normalize(value)))
            {
                OnPropertyChanged(nameof(HasCustomDownloadColor));
            }
        }
    }

    public string? UploadColor
    {
        get => _uploadColor;
        set
        {
            if (SetProperty(ref _uploadColor, HexColor.Normalize(value)))
            {
                OnPropertyChanged(nameof(HasCustomUploadColor));
            }
        }
    }

    public bool HasCustomDownloadColor => _downloadColor is not null;

    public bool HasCustomUploadColor => _uploadColor is not null;

    // Language (applies immediately, without Save; takes effect after a restart)

    public AppLanguage Language
    {
        get => _language;
        set
        {
            try
            {
                _store.Update(s => s with { Language = value });
                _language = value;
                LanguageError = null;
            }
            catch (SettingsSaveException error)
            {
                LanguageError = string.Format(CultureInfo.CurrentCulture, Strings.CouldNotSaveSettingsFormat, error.InnerException?.Message);
            }
            OnPropertiesChanged(nameof(Language), nameof(NeedsRestart));
        }
    }

    /// <summary>The chosen language differs from the one this run started with.</summary>
    public bool NeedsRestart => _language != _launchLanguage;

    public string? LanguageError
    {
        get => _languageError;
        private set => SetProperty(ref _languageError, value);
    }

    // Startup (applies immediately, without Save)

    public bool OpenAtLogin
    {
        get => _launchAtLogin?.IsEnabled ?? false;
        set
        {
            if (_launchAtLogin is null)
            {
                return;
            }
            try
            {
                _launchAtLogin.SetEnabled(value);
                StartupError = null;
            }
            catch (Exception error) when (error is UnauthorizedAccessException or System.Security.SecurityException or System.IO.IOException)
            {
                StartupError = string.Format(CultureInfo.CurrentCulture, Strings.StartupErrorFormat, error.Message);
            }
            OnPropertyChanged();
        }
    }

    public string? StartupError
    {
        get => _startupError;
        private set => SetProperty(ref _startupError, value);
    }

    // Status

    /// <summary>Why the current draft can't connect (shown in the UI).</summary>
    public string? ConnectionProblem =>
        string.IsNullOrWhiteSpace(_host) ? Strings.EnterDeviceAddress
        : !DeviceAddress.TryParse(_host, out _) ? Strings.EnterValidAddress
        : Port is null ? Strings.PortRange
        : Security.Problem is { } problem ? ProblemText.Describe(problem)
        : null;

    public bool CanConnect => ConnectionProblem is null;

    public bool CanSave => CanConnect && _selected.Count > 0;

    public bool IsWorking => _activityKind == ActivityKind.Working;

    public ActivityKind ActivityKind
    {
        get => _activityKind;
        private set
        {
            if (SetProperty(ref _activityKind, value))
            {
                OnPropertyChanged(nameof(IsWorking));
                ScanCommand.NotifyCanExecuteChanged();
                TestCommand.NotifyCanExecuteChanged();
            }
        }
    }

    public string ActivityText
    {
        get => _activityText;
        private set => SetProperty(ref _activityText, value);
    }

    private SecuritySettings Security => new()
    {
        Mode = _mode,
        Community = _community,
        AuthProtocol = _authProtocol,
        AuthPassword = _authPassword,
        PrivProtocol = _privProtocol,
        PrivPassword = _privPassword,
    };

    /// <summary>Selects / deselects an interface, keeping the list order.</summary>
    public void SetSelected(string name, bool selected)
    {
        var names = selected ? [.. _selected.Where(n => n != name), name] : _selected.Where(n => n != name).ToList();
        var order = InterfaceChoices.Select(c => c.Name).ToList();
        _selected.Clear();
        _selected.AddRange(order.Where(names.Contains).Concat(names.Where(n => !order.Contains(n))));

        foreach (var choice in InterfaceChoices)
        {
            choice.SetSelectedSilently(_selected.Contains(choice.Name));
        }
        // The floating window interface must be one of the selected ones.
        if (!_selected.Contains(_floatingInterface))
        {
            FloatingInterface = _selected.FirstOrDefault() ?? string.Empty;
        }
        OnPropertiesChanged(nameof(SelectedInterfaces), nameof(HasMultipleSelected), nameof(CanSave));
        SaveCommand.NotifyCanExecuteChanged();
    }

    public async Task ScanAsync()
    {
        await RunAsync(Strings.Scanning, async (client, cancellationToken) =>
        {
            var found = await client.ListInterfacesAsync(cancellationToken);
            return () =>
            {
                _scanned = [.. found];
                RebuildChoices();
                if (_selected.Count == 0 && found.FirstOrDefault(i => i.Name.StartsWith("pppoe-out", StringComparison.Ordinal)) is { } wan)
                {
                    SetSelected(wan.Name, true);
                }
                return found.Count == 0
                    ? (ActivityKind.Failure, Strings.NoInterfacesFound)
                    : (ActivityKind.Success, string.Format(CultureInfo.CurrentCulture, Strings.InterfacesFoundFormat, found.Count));
            };
        });
    }

    public async Task TestAsync()
    {
        var names = _selected.ToList();
        await RunAsync(Strings.Testing, async (client, cancellationToken) =>
        {
            var binds = await client.GetAsync([SnmpOids.SysUpTime], cancellationToken);
            var message = new StringBuilder(Strings.ConnectedSentence);
            if (binds.Count > 0 && binds[0].Value is SnmpValue.TimeTicks ticks)
            {
                message.Append(' ').Append(string.Format(
                    CultureInfo.CurrentCulture, Strings.DeviceUpSentenceFormat, UptimeText.Format(Uptime.FromTimeTicks(ticks.Value))));
            }
            var indexes = await client.ResolveInterfaceIndexesAsync(names, cancellationToken);
            foreach (var (name, index) in names.Zip(indexes))
            {
                message.Append(' ').Append(string.Format(CultureInfo.CurrentCulture, Strings.IfIndexFormat, name, index));
            }
            return () => (ActivityKind.Success, message.ToString());
        });
    }

    public void Save()
    {
        if (!CanSave || Port is not { } port || !DeviceAddress.TryParse(_host, out var address))
        {
            return;
        }
        var settings = _store.Current with
        {
            Host = address.Host,
            Port = port,
            Security = Security,
            InterfaceNames = [.. _selected],
            FloatingInterface = _floatingInterface,
            RefreshSeconds = _refreshSeconds,
            UnitStyle = _unitStyle,
            ChartsWhenMultiple = _chartsWhenMultiple,
            DownloadColor = _downloadColor,
            UploadColor = _uploadColor,
            ShowFloatingWindow = _showFloatingWindow,
            FloatingAlwaysOnTop = _floatingAlwaysOnTop,
            FloatingOpacity = _floatingOpacityPercent / 100,
        };
        try
        {
            _store.Save(settings);
            SetActivity(ActivityKind.Success, Strings.Saved);
            Saved?.Invoke(this, EventArgs.Empty);
        }
        catch (SettingsSaveException error)
        {
            SetActivity(
                ActivityKind.Failure,
                error.Failure == SettingsSaveFailure.Credentials
                    ? Strings.CouldNotSaveCredentials
                    : string.Format(CultureInfo.CurrentCulture, Strings.CouldNotSaveSettingsFormat, error.InnerException?.Message));
        }
    }

    public void CancelWork()
    {
        _work?.Cancel();
        _work?.Dispose();
        _work = null;
    }

    public void Dispose() => CancelWork();

    /// <summary>Creates a temporary SNMP client to run <paramref name="work"/>, then applies the result on the UI thread.</summary>
    private async Task RunAsync(
        string progress,
        Func<SnmpClient, CancellationToken, Task<Func<(ActivityKind Kind, string Text)>>> work)
    {
        if (!CanConnect || Port is not { } port || !DeviceAddress.TryParse(_host, out var address))
        {
            return;
        }
        CancelWork();
        var cancellation = new CancellationTokenSource();
        _work = cancellation;
        SetActivity(ActivityKind.Working, progress);

        (ActivityKind Kind, string Text) outcome;
        try
        {
            var transport = await _makeTransport(address.Host, port, cancellation.Token);
            var client = new SnmpClient(transport, Security.ToSnmpSecurity(), _timeout);
            await using (client)
            {
                outcome = (await work(client, cancellation.Token))();
            }
        }
        catch (OperationCanceledException) when (cancellation.IsCancellationRequested)
        {
            return;
        }
        catch (Exception error) when (error is SnmpException or System.Net.Sockets.SocketException)
        {
            outcome = (ActivityKind.Failure, ErrorText.Describe(error));
        }

        if (!cancellation.IsCancellationRequested)
        {
            SetActivity(outcome.Kind, outcome.Text);
        }
    }

    private void SetActivity(ActivityKind kind, string text)
    {
        ActivityText = text;
        ActivityKind = kind;
    }

    private void RebuildChoices()
    {
        var scanned = _scanned.Select(i => i.Name).ToList();
        InterfaceChoices.Clear();
        foreach (var name in _selected.Where(n => !scanned.Contains(n)).Concat(scanned))
        {
            InterfaceChoices.Add(new InterfaceChoice(name, _selected.Contains(name), choice => SetSelected(choice.Name, choice.IsSelected)));
        }
        OnPropertyChanged(nameof(HasInterfaceChoices));
    }

    /// <summary>Fields that affect whether a connection is possible: update the check result and button state when they change.</summary>
    private bool SetDraft<T>(ref T field, T value, [System.Runtime.CompilerServices.CallerMemberName] string? propertyName = null)
    {
        if (!SetProperty(ref field, value, propertyName))
        {
            return false;
        }
        OnPropertiesChanged(nameof(ConnectionProblem), nameof(CanConnect), nameof(CanSave));
        ScanCommand.NotifyCanExecuteChanged();
        TestCommand.NotifyCanExecuteChanged();
        SaveCommand.NotifyCanExecuteChanged();
        return true;
    }
}
