using System.Globalization;
using System.Windows;
using Anemobit.Core.Traffic;
using Anemobit.Core.Transport;
using Anemobit.Settings;
using Anemobit.Theming;
using Anemobit.Tray;
using Anemobit.ViewModels;
using Anemobit.Views;

namespace Anemobit;

/// <summary>
/// Notification area app: floating rate window + notification area icon; the main window, Settings and About open as needed. Only one instance is allowed:
/// a second launch tells the first instance to open the main window, then exits. An instance started by <see cref="Relaunch"/> instead waits for the previous one to exit.
/// </summary>
public partial class App : Application, IDisposable
{
    private const string InstanceMutexName = @"Local\Anemobit.SingleInstance";
    private const string ActivateEventName = @"Local\Anemobit.Activate";

    private Mutex? _instanceMutex;
    private bool _ownsInstanceMutex;
    private EventWaitHandle? _activateEvent;
    private RegisteredWaitHandle? _activateWait;
    private SettingsStore _settings = null!;
    private AppLanguage _launchLanguage;
    private ThemeManager _theme = null!;
    private TrafficViewModel _traffic = null!;
    private AppCommands _commands = null!;
    private FloatingRateWindow _floating = null!;
    private TrayFlyout _flyout = null!;
    private TrayIcon _tray = null!;
    private MainWindow? _mainWindow;
    private SettingsWindow? _settingsWindow;
    private AboutWindow? _aboutWindow;

    protected override void OnStartup(StartupEventArgs e)
    {
        base.OnStartup(e);

        // Called by the installer on uninstall: only clean up the current user's data, without opening any window.
        if (e.Args.Contains(UserDataCleanup.Argument, StringComparer.OrdinalIgnoreCase))
        {
            UserDataCleanup.ForCurrentUser().Run();
            Shutdown();
            return;
        }

        var relaunched = e.Args.Contains(Relaunch.Argument, StringComparer.OrdinalIgnoreCase);
        _instanceMutex = new Mutex(initiallyOwned: true, InstanceMutexName, out var isFirstInstance);
        if (!isFirstInstance && relaunched)
        {
            // The previous instance is quitting after Restart Now: wait for it instead of handing off.
            isFirstInstance = WaitForPreviousInstance(_instanceMutex);
        }
        _ownsInstanceMutex = isFirstInstance;
        if (!isFirstInstance)
        {
            using var activate = EventWaitHandle.OpenExisting(ActivateEventName);
            activate.Set();
            Shutdown();
            return;
        }
        _activateEvent = new EventWaitHandle(false, EventResetMode.AutoReset, ActivateEventName);
        _activateWait = ThreadPool.RegisterWaitForSingleObject(
            _activateEvent, (_, _) => Dispatcher.BeginInvoke(OpenMainWindow), null, Timeout.Infinite, executeOnlyOnce: false);

        _settings = new SettingsStore(SettingsStore.DefaultPath, new WindowsCredentialStore());
        _launchLanguage = _settings.Current.Language;
        if (_launchLanguage.ToCulture() is { } culture)
        {
            // Before any window or menu is created, so every resource lookup uses the chosen language.
            CultureInfo.DefaultThreadCurrentUICulture = culture;
            CultureInfo.CurrentUICulture = culture;
        }
        _theme = new ThemeManager(this);
        _theme.SetTrafficColors(_settings.Current.DownloadColor, _settings.Current.UploadColor);
        _traffic = new TrafficViewModel(_settings, configuration => new SnmpTrafficPoller(configuration));
        _commands = new AppCommands(OpenMainWindow, OpenSettings, OpenAbout, SetFloatingWindowVisible, Quit);

        _flyout = new TrayFlyout(_traffic, _commands);
        _floating = new FloatingRateWindow(_traffic, _settings, _commands, anchor => _flyout.Toggle(anchor));
        _tray = new TrayIcon(_traffic, _commands, () => _floating.IsVisible, anchor => _flyout.Toggle(anchor));

        _settings.Changed += (_, _) => ApplySettings();
        _theme.Changed += (_, _) => _floating.ApplySettings();
        ApplySettings();
        _traffic.Start();

        var launchedAtLogin = e.Args.Contains(LaunchAtLogin.StartupArgument, StringComparer.OrdinalIgnoreCase);
        if (relaunched)
        {
            // Back to where Restart Now was pressed, now in the new language.
            OpenSettings();
        }
        else if (!_settings.Current.IsConfigured && !launchedAtLogin)
        {
            OpenMainWindow();
        }
        if (!_settings.Current.TrayHintShown)
        {
            _tray.ShowFirstRunHint();
            TryUpdateSettings(s => s with { TrayHintShown = true });
        }
    }

    public void Dispose()
    {
        _traffic?.Dispose();
        _tray?.Dispose();
        _theme?.Dispose();
        _activateWait?.Unregister(null);
        _activateEvent?.Dispose();
        // Released last, so a relaunched instance starts only after this one has removed its notification area icon.
        if (_ownsInstanceMutex)
        {
            _instanceMutex?.ReleaseMutex();
            _ownsInstanceMutex = false;
        }
        _instanceMutex?.Dispose();
        GC.SuppressFinalize(this);
    }

    protected override void OnExit(ExitEventArgs e)
    {
        Dispose();
        base.OnExit(e);
    }

    /// <summary>Applies settings changes (saved, or from floating window actions) to the display.</summary>
    private void ApplySettings()
    {
        var settings = _settings.Current;
        _theme.SetTrafficColors(settings.DownloadColor, settings.UploadColor);
        _floating.ApplySettings();
        if (settings.ShowFloatingWindow && !_floating.IsVisible)
        {
            _floating.Show();
        }
        else if (!settings.ShowFloatingWindow && _floating.IsVisible)
        {
            _floating.Hide();
        }
    }

    private void OpenMainWindow()
    {
        _mainWindow ??= CreateMainWindow();
        Present(_mainWindow);
    }

    private MainWindow CreateMainWindow()
    {
        var window = new MainWindow(_traffic, OpenSettings);
        window.Closed += (_, _) => _mainWindow = null;
        return window;
    }

    private void OpenSettings()
    {
        if (_settingsWindow is null)
        {
            var model = new SettingsViewModel(
                _settings,
                async (host, port, cancellationToken) => await UdpSocketTransport.ConnectAsync(host, port, cancellationToken),
                LaunchAtLogin.ForCurrentUser(),
                _launchLanguage,
                Restart);
            _settingsWindow = new SettingsWindow(model);
            _settingsWindow.Closed += (_, _) => _settingsWindow = null;
        }
        Present(_settingsWindow);
    }

    private void OpenAbout()
    {
        if (_aboutWindow is null)
        {
            _aboutWindow = new AboutWindow();
            _aboutWindow.Closed += (_, _) => _aboutWindow = null;
        }
        Present(_aboutWindow);
    }

    private void SetFloatingWindowVisible(bool visible) => TryUpdateSettings(s => s with { ShowFloatingWindow = visible });

    private void Quit() => Shutdown();

    private void Restart()
    {
        // If the new instance couldn't start, keep running rather than leave the user with no app.
        if (Relaunch.TryStart())
        {
            Shutdown();
        }
    }

    /// <summary>Waits until the previous instance releases the single-instance mutex; true when this instance now owns it.</summary>
    private static bool WaitForPreviousInstance(Mutex mutex)
    {
        try
        {
            return mutex.WaitOne(Relaunch.Timeout);
        }
        catch (AbandonedMutexException)
        {
            // The previous instance exited without releasing it (e.g. it was ended); the mutex is now ours.
            return true;
        }
    }

    private void TryUpdateSettings(Func<AppSettings, AppSettings> change)
    {
        try
        {
            _settings.Update(change);
        }
        catch (SettingsSaveException)
        {
            // If the settings file can't be written, keep things as they are (e.g. the floating window stays visible); don't let the app exit.
        }
    }

    private static void Present(Window window)
    {
        if (window.WindowState == WindowState.Minimized)
        {
            window.WindowState = WindowState.Normal;
        }
        window.Show();
        window.Activate();
    }
}
