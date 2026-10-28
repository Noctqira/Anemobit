using Anemobit.Core.Formatting;
using Anemobit.Core.Traffic;

namespace Anemobit.Settings;

/// <summary>
/// User settings (immutable). Non-sensitive settings are stored as JSON (<see cref="SettingsStore"/>); the community and SNMPv3 passwords in Windows Credential Manager.
/// </summary>
public sealed record AppSettings
{
    public const int DefaultPort = 161;

    /// <summary>Refresh rate options (seconds).</summary>
    public static IReadOnlyList<int> RefreshIntervals { get; } = [1, 5, 10];

    public string Host { get; init; } = string.Empty;

    public int Port { get; init; } = DefaultPort;

    /// <summary>Monitored interfaces (ifName), in the order shown in Settings.</summary>
    public IReadOnlyList<string> InterfaceNames { get; init; } = [];

    /// <summary>Interface shown in the floating window (and the notification area flyout); read it through <see cref="EffectiveFloatingInterface"/>.</summary>
    public string FloatingInterface { get; init; } = string.Empty;

    public SecuritySettings Security { get; init; } = new();

    public int RefreshSeconds { get; init; } = 1;

    public RateUnitStyle UnitStyle { get; init; } = RateUnitStyle.Bits;

    /// <summary>Whether interface cards show charts when monitoring two or more interfaces (always shown for a single interface).</summary>
    public bool ChartsWhenMultiple { get; init; }

    /// <summary>Custom download color (#RRGGBB); null means the theme default (follows light / dark).</summary>
    public string? DownloadColor { get; init; }

    /// <summary>Custom upload color (#RRGGBB); null means the theme default.</summary>
    public string? UploadColor { get; init; }

    /// <summary>Display language; applied at launch. Changed from Settings immediately, without Save.</summary>
    public AppLanguage Language { get; init; }

    public bool ShowFloatingWindow { get; init; } = true;

    public bool FloatingAlwaysOnTop { get; init; } = true;

    /// <summary>Floating window background opacity (0–1); semi-transparent by default.</summary>
    public double FloatingOpacity { get; init; } = 0.7;

    public FloatingWindowPosition? FloatingPosition { get; init; }

    /// <summary>Whether the first-launch notification area tip has been shown.</summary>
    public bool TrayHintShown { get; init; }

    public bool IsConfigured => Host.Length > 0 && InterfaceNames.Count > 0 && Security.Problem is null;

    /// <summary>Interface actually shown in the floating window: falls back to the first one when the chosen interface isn't monitored.</summary>
    public string? EffectiveFloatingInterface =>
        InterfaceNames.Contains(FloatingInterface) ? FloatingInterface : InterfaceNames.Count > 0 ? InterfaceNames[0] : null;

    public PollerConfiguration? ToPollerConfiguration() => IsConfigured
        ? new PollerConfiguration
        {
            Host = Host,
            Port = Port,
            Security = Security.ToSnmpSecurity(),
            InterfaceNames = InterfaceNames,
            Interval = TimeSpan.FromSeconds(RefreshSeconds),
        }
        : null;
}
