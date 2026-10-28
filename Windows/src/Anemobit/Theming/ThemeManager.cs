using System.Windows;
using System.Windows.Media;
using Microsoft.Win32;
using Anemobit.Settings;

namespace Anemobit.Theming;

/// <summary>
/// Switches colors with the Windows app mode (light / dark) and applies the user's custom download / upload colors.
/// Fluent controls themselves are handled by <see cref="Application.ThemeMode"/>; this manages Anemobit's own color resources.
/// </summary>
public sealed class ThemeManager : IDisposable
{
    public const string DownloadBrushKey = "DownloadBrush";
    public const string UploadBrushKey = "UploadBrush";
    public const string DownloadColorKey = "DownloadColor";
    public const string UploadColorKey = "UploadColor";

    private static readonly Uri LightPalette = new("pack://application:,,,/Anemobit;component/Theming/Palette.Light.xaml");
    private static readonly Uri DarkPalette = new("pack://application:,,,/Anemobit;component/Theming/Palette.Dark.xaml");

    private readonly Application _application;
    private ResourceDictionary? _palette;
    private string? _downloadOverride;
    private string? _uploadOverride;

    public ThemeManager(Application application)
    {
        _application = application;
        SystemEvents.UserPreferenceChanged += OnUserPreferenceChanged;
        Apply();
    }

    public event EventHandler? Changed;

    public bool IsDark { get; private set; }

    /// <summary>Whether Windows' "Choose your app mode" is dark.</summary>
    public static bool SystemUsesDarkMode()
    {
        using var key = Registry.CurrentUser.OpenSubKey(@"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize");
        return key?.GetValue("AppsUseLightTheme") is int light && light == 0;
    }

    /// <summary>Applies custom colors when settings change (null means the theme default).</summary>
    public void SetTrafficColors(string? download, string? upload)
    {
        _downloadOverride = download;
        _uploadOverride = upload;
        ApplyTrafficColors();
        Changed?.Invoke(this, EventArgs.Empty);
    }

    public void Dispose() => SystemEvents.UserPreferenceChanged -= OnUserPreferenceChanged;

    private void OnUserPreferenceChanged(object sender, UserPreferenceChangedEventArgs e)
    {
        if (e.Category == UserPreferenceCategory.General && SystemUsesDarkMode() != IsDark)
        {
            _application.Dispatcher.BeginInvoke(() =>
            {
                Apply();
                Changed?.Invoke(this, EventArgs.Empty);
            });
        }
    }

    private void Apply()
    {
        IsDark = SystemUsesDarkMode();
        var dictionaries = _application.Resources.MergedDictionaries;
        if (_palette is not null)
        {
            dictionaries.Remove(_palette);
        }
        _palette = new ResourceDictionary { Source = IsDark ? DarkPalette : LightPalette };
        dictionaries.Add(_palette);
        ApplyTrafficColors();
    }

    private void ApplyTrafficColors()
    {
        var download = HexColor.ToColor(_downloadOverride) ?? (Color)_palette!["DefaultDownloadColor"];
        var upload = HexColor.ToColor(_uploadOverride) ?? (Color)_palette!["DefaultUploadColor"];
        var resources = _application.Resources;
        resources[DownloadColorKey] = download;
        resources[UploadColorKey] = upload;
        resources[DownloadBrushKey] = Frozen(new SolidColorBrush(download));
        resources[UploadBrushKey] = Frozen(new SolidColorBrush(upload));
    }

    private static SolidColorBrush Frozen(SolidColorBrush brush)
    {
        brush.Freeze();
        return brush;
    }
}
