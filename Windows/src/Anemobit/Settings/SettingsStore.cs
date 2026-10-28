using System.IO;
using System.Text.Json;
using Anemobit.Core.Formatting;
using Anemobit.Core.Snmp;

namespace Anemobit.Settings;

/// <summary>
/// Reads and writes settings: non-sensitive settings in <c>%APPDATA%\Anemobit\settings.json</c>; the community and passwords in <see cref="ICredentialStore"/>.
/// Raises <see cref="Changed"/> when <see cref="Current"/> changes (on the thread that called <see cref="Save"/>).
/// </summary>
public sealed class SettingsStore
{
    public const string CommunityName = "community";
    public const string AuthPasswordName = "authPassword";
    public const string PrivPasswordName = "privPassword";

    private readonly string _path;
    private readonly ICredentialStore _credentials;

    public SettingsStore(string path, ICredentialStore credentials)
    {
        _path = path;
        _credentials = credentials;
        Current = Load();
    }

    public event EventHandler? Changed;

    public static string DefaultPath { get; } =
        Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData), "Anemobit", "settings.json");

    public AppSettings Current { get; private set; }

    /// <summary>Saves all settings. Throws <see cref="SettingsSaveException"/> if the settings file or Credential Manager write fails; <see cref="Current"/> is unchanged.</summary>
    public void Save(AppSettings settings)
    {
        ArgumentNullException.ThrowIfNull(settings);
        WriteDocument(settings);

        // Keep the passwords when switching to another mode, so switching back is easy.
        var security = settings.Security;
        var stored = Store(CommunityName, security.Community)
            & Store(AuthPasswordName, security.AuthPassword)
            & Store(PrivPasswordName, security.PrivPassword);
        if (!stored)
        {
            throw new SettingsSaveException(SettingsSaveFailure.Credentials);
        }

        Current = settings;
        Changed?.Invoke(this, EventArgs.Empty);
    }

    /// <summary>Changes only non-sensitive settings (e.g. the floating window position) and writes immediately.</summary>
    public void Update(Func<AppSettings, AppSettings> change)
    {
        ArgumentNullException.ThrowIfNull(change);
        var updated = change(Current);
        if (updated == Current)
        {
            return;
        }
        WriteDocument(updated);
        Current = updated;
        Changed?.Invoke(this, EventArgs.Empty);
    }

    private AppSettings Load()
    {
        var document = ReadDocument();
        return new AppSettings
        {
            Host = document.Device.Host,
            Port = document.Device.Port is > 0 and <= 65_535 ? document.Device.Port : AppSettings.DefaultPort,
            InterfaceNames = [.. document.Device.InterfaceNames.Distinct(StringComparer.Ordinal)],
            FloatingInterface = document.FloatingWindow.Interface,
            Security = new SecuritySettings
            {
                Mode = ParseMode(document.Snmp.SecurityMode),
                Community = _credentials.Read(CommunityName) ?? string.Empty,
                AuthProtocol = ParseEnum(document.Snmp.AuthProtocol, SnmpAuthProtocol.Sha1),
                AuthPassword = _credentials.Read(AuthPasswordName) ?? string.Empty,
                PrivProtocol = ParseEnum(document.Snmp.PrivProtocol, SnmpPrivProtocol.Aes),
                PrivPassword = _credentials.Read(PrivPasswordName) ?? string.Empty,
            },
            RefreshSeconds = AppSettings.RefreshIntervals.Contains(document.Display.RefreshSeconds) ? document.Display.RefreshSeconds : 1,
            UnitStyle = ParseEnum(document.Display.UnitStyle, RateUnitStyle.Bits),
            ChartsWhenMultiple = document.Display.ChartsWhenMultiple,
            DownloadColor = HexColor.Normalize(document.Display.DownloadColor),
            UploadColor = HexColor.Normalize(document.Display.UploadColor),
            Language = AppLanguageExtensions.FromCultureName(document.Display.Language),
            ShowFloatingWindow = document.FloatingWindow.Visible,
            FloatingAlwaysOnTop = document.FloatingWindow.AlwaysOnTop,
            FloatingOpacity = Math.Clamp(document.FloatingWindow.Opacity, 0, 1),
            FloatingPosition = document.FloatingWindow is { Monitor: { } monitor, X: { } x, Y: { } y }
                ? new FloatingWindowPosition(monitor, x, y)
                : null,
            TrayHintShown = document.FloatingWindow.TrayHintShown,
        };
    }

    private SettingsDocument ReadDocument()
    {
        try
        {
            using var stream = File.OpenRead(_path);
            return JsonSerializer.Deserialize(stream, SettingsJsonContext.Default.SettingsDocument) ?? new SettingsDocument();
        }
        catch (Exception error) when (error is IOException or UnauthorizedAccessException or JsonException)
        {
            // No settings file (first launch) or corrupt contents: use the defaults.
            return new SettingsDocument();
        }
    }

    private void WriteDocument(AppSettings settings)
    {
        var document = new SettingsDocument
        {
            Device = new()
            {
                Host = settings.Host,
                Port = settings.Port,
                InterfaceNames = [.. settings.InterfaceNames],
            },
            Snmp = new()
            {
                SecurityMode = FormatMode(settings.Security.Mode),
                AuthProtocol = FormatEnum(settings.Security.AuthProtocol),
                PrivProtocol = FormatEnum(settings.Security.PrivProtocol),
            },
            Display = new()
            {
                RefreshSeconds = settings.RefreshSeconds,
                UnitStyle = FormatEnum(settings.UnitStyle),
                ChartsWhenMultiple = settings.ChartsWhenMultiple,
                DownloadColor = settings.DownloadColor,
                UploadColor = settings.UploadColor,
                Language = settings.Language.CultureName(),
            },
            FloatingWindow = new()
            {
                Visible = settings.ShowFloatingWindow,
                AlwaysOnTop = settings.FloatingAlwaysOnTop,
                Opacity = settings.FloatingOpacity,
                Interface = settings.FloatingInterface,
                Monitor = settings.FloatingPosition?.Monitor,
                X = settings.FloatingPosition?.X,
                Y = settings.FloatingPosition?.Y,
                TrayHintShown = settings.TrayHintShown,
            },
        };

        try
        {
            Directory.CreateDirectory(Path.GetDirectoryName(_path)!);
            // Write a temporary file and then replace, so a crash mid-write doesn't leave a corrupt settings file.
            var temporary = _path + ".tmp";
            using (var stream = File.Create(temporary))
            {
                JsonSerializer.Serialize(stream, document, SettingsJsonContext.Default.SettingsDocument);
            }
            File.Move(temporary, _path, overwrite: true);
        }
        catch (Exception error) when (error is IOException or UnauthorizedAccessException)
        {
            throw new SettingsSaveException(SettingsSaveFailure.File, error);
        }
    }

    private bool Store(string name, string secret) =>
        secret.Length == 0 ? _credentials.Delete(name) : _credentials.Write(name, secret);

    private static SecurityMode ParseMode(string? text) => text switch
    {
        "authorized" => SecurityMode.Authorized,
        "private" => SecurityMode.Private,
        _ => SecurityMode.None,
    };

    private static string FormatMode(SecurityMode mode) => mode switch
    {
        SecurityMode.Authorized => "authorized",
        SecurityMode.Private => "private",
        _ => "none",
    };

    private static T ParseEnum<T>(string? text, T fallback)
        where T : struct, Enum =>
        Enum.TryParse<T>(text, ignoreCase: true, out var value) && Enum.IsDefined(value) ? value : fallback;

    private static string FormatEnum<T>(T value)
        where T : struct, Enum =>
        value.ToString().ToLowerInvariant();
}
