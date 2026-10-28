using System.Windows;
using System.Windows.Input;
using System.Windows.Media;
using Anemobit.Core.Formatting;
using Anemobit.Settings;
using Anemobit.ViewModels;
using Screen = System.Windows.Forms.Screen;

namespace Anemobit.Views;

/// <summary>
/// Floating rate window: can be dragged anywhere and remembers its position (per monitor); doesn't take focus or appear in the taskbar or Alt+Tab.
/// A click opens the notification area flyout; right-click opens the menu.
/// </summary>
public partial class FloatingRateWindow : Window
{
    /// <summary>Fully transparent pixels let the mouse through, so the background keeps at least 1/255.</summary>
    private const byte MinimumAlpha = 1;
    private const int DefaultMargin = 16;

    private readonly SettingsStore _settings;
    private readonly AppCommands _commands;
    private readonly Action<System.Drawing.Point> _toggleFlyout;

    public FloatingRateWindow(TrafficViewModel traffic, SettingsStore settings, AppCommands commands, Action<System.Drawing.Point> toggleFlyout)
    {
        InitializeComponent();
        DataContext = traffic;
        _settings = settings;
        _commands = commands;
        _toggleFlyout = toggleFlyout;
        SourceInitialized += (_, _) => NativeMethods.MakeToolWindow(this, noActivate: true);
        Loaded += (_, _) => RestorePosition();
    }

    /// <summary>Updates when settings or the theme change: always on top, background opacity, value column widths.</summary>
    public void ApplySettings()
    {
        var settings = _settings.Current;
        Topmost = settings.FloatingAlwaysOnTop;
        var baseColor = (Color)FindResource("FloatingBaseColor");
        var alpha = (byte)Math.Max(MinimumAlpha, Math.Round(settings.FloatingOpacity * 255));
        var background = new SolidColorBrush(Color.FromArgb(alpha, baseColor.R, baseColor.G, baseColor.B));
        background.Freeze();
        Panel.Background = background;

        // Fixed column widths: the window size doesn't jump when numbers change.
        var widest = RateFormatter.AllUnits(settings.UnitStyle).Max(unit => DownText.MeasureTextWidth($"↓ 9999.9 {unit}"));
        DownText.MinWidth = UpText.MinWidth = Math.Ceiling(widest + (DownText.HaloWidth * 2));
    }

    /// <summary>Returns to the remembered position; if that monitor is gone or the position is off screen, goes to the bottom right of the primary monitor.</summary>
    private void RestorePosition()
    {
        var (_, _, width, height) = NativeMethods.Bounds(this);
        if (_settings.Current.FloatingPosition is { } saved
            && Screen.AllScreens.FirstOrDefault(s => s.DeviceName == saved.Monitor) is { } screen
            && screen.WorkingArea.Contains(new System.Drawing.Rectangle(saved.X, saved.Y, Math.Min(width, 40), Math.Min(height, 20))))
        {
            NativeMethods.MoveTo(this, saved.X, saved.Y);
            return;
        }

        var area = Screen.PrimaryScreen?.WorkingArea ?? new System.Drawing.Rectangle(0, 0, 1280, 720);
        var margin = (int)(DefaultMargin * VisualTreeHelper.GetDpi(this).DpiScaleX);
        NativeMethods.MoveTo(this, area.Right - width - margin, area.Bottom - height - margin);
    }

    /// <summary>Drag to move; if the position didn't change on release, treat it as a click.</summary>
    private void OnMouseLeftButtonDown(object sender, MouseButtonEventArgs e)
    {
        var before = NativeMethods.Bounds(this);
        DragMove();
        var after = NativeMethods.Bounds(this);
        if (Math.Abs(after.Left - before.Left) <= 2 && Math.Abs(after.Top - before.Top) <= 2)
        {
            _toggleFlyout(new System.Drawing.Point(after.Left + after.Width, after.Top));
            return;
        }

        var monitor = Screen.FromPoint(new System.Drawing.Point(after.Left + (after.Width / 2), after.Top + (after.Height / 2))).DeviceName;
        try
        {
            _settings.Update(s => s with { FloatingPosition = new FloatingWindowPosition(monitor, after.Left, after.Top) });
        }
        catch (SettingsSaveException)
        {
            // Failing to remember the position doesn't affect use; the next launch returns to the default position.
        }
    }

    private void OnMouseRightButtonUp(object sender, MouseButtonEventArgs e)
    {
        var menu = AppMenu.Create(_commands, floatingVisible: true);
        menu.PlacementTarget = Panel;
        menu.IsOpen = true;
        e.Handled = true;
    }
}
