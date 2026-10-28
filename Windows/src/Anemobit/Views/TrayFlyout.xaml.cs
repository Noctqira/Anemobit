using System.Windows;
using System.Windows.Media;
using Anemobit.ViewModels;
using Screen = System.Windows.Forms.Screen;

namespace Anemobit.Views;

/// <summary>
/// Notification area flyout. Opens near the given anchor (physical pixels), kept within that monitor's work area; hides when it loses focus.
/// </summary>
public partial class TrayFlyout : Window
{
    private const int Gap = 12;
    private readonly AppCommands _commands;
    private DateTime _hiddenAt;

    public TrayFlyout(TrafficViewModel traffic, AppCommands commands)
    {
        InitializeComponent();
        AcrylicBackdrop.Attach(this);
        DataContext = traffic;
        _commands = commands;
        SourceInitialized += (_, _) =>
        {
            NativeMethods.MakeToolWindow(this, noActivate: false);
            NativeMethods.UseRoundedCorners(this);
        };
        Deactivated += (_, _) => HideFlyout();
    }

    /// <summary>
    /// Toggles visibility. Clicking the notification area icon first hides the window through focus loss, and that same click shouldn't reopen it, so toggles right after hiding are ignored.
    /// </summary>
    public void Toggle(System.Drawing.Point anchor)
    {
        if (IsVisible)
        {
            HideFlyout();
            return;
        }
        if ((DateTime.UtcNow - _hiddenAt).TotalMilliseconds < 300)
        {
            return;
        }
        ShowNear(anchor);
    }

    /// <summary>Opens near the anchor: upward when the anchor is in the bottom half of the screen, otherwise downward; horizontally the anchor is the right edge.</summary>
    public void ShowNear(System.Drawing.Point anchor)
    {
        Opacity = 0;
        Show();
        UpdateLayout();

        var scale = VisualTreeHelper.GetDpi(this).DpiScaleX;
        var (_, _, width, height) = NativeMethods.Bounds(this);
        var area = Screen.FromPoint(anchor).WorkingArea;
        var gap = (int)(Gap * scale);
        var x = Math.Clamp(anchor.X - width + gap, area.Left + gap, area.Right - width - gap);
        var below = anchor.Y < area.Top + (area.Height / 2);
        var y = below ? anchor.Y + gap : anchor.Y - height - gap;
        y = Math.Clamp(y, area.Top + gap, area.Bottom - height - gap);
        NativeMethods.MoveTo(this, x, y);

        Opacity = 1;
        Activate();
        NativeMethods.BringToForeground(this);
    }

    private void HideFlyout()
    {
        if (!IsVisible)
        {
            return;
        }
        Hide();
        _hiddenAt = DateTime.UtcNow;
    }

    private void OnOpenMainWindow(object sender, RoutedEventArgs e)
    {
        HideFlyout();
        _commands.OpenMainWindow();
    }

    private void OnOpenSettings(object sender, RoutedEventArgs e)
    {
        HideFlyout();
        _commands.OpenSettings();
    }

    private void OnQuit(object sender, RoutedEventArgs e) => _commands.Quit();
}
