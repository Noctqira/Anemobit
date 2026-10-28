using System.ComponentModel;
using System.Windows;
using Anemobit.Resources;
using Anemobit.ViewModels;
using Anemobit.Views;
using Forms = System.Windows.Forms;

namespace Anemobit.Tray;

/// <summary>
/// Notification area icon (.NET's built-in WinForms NotifyIcon): always present, and the way back in when the floating window is hidden.
/// A click opens the flyout; the right-click menu is the same as the floating window's. The tooltip shows the current rates.
/// </summary>
public sealed class TrayIcon : IDisposable
{
    private readonly Forms.NotifyIcon _icon;
    private readonly TrafficViewModel _traffic;
    private readonly AppCommands _commands;
    private readonly Func<bool> _floatingVisible;
    private readonly Action<System.Drawing.Point> _toggleFlyout;
    private readonly System.Drawing.Icon _image;
    private Window? _menuHost;

    public TrayIcon(TrafficViewModel traffic, AppCommands commands, Func<bool> floatingVisible, Action<System.Drawing.Point> toggleFlyout)
    {
        _traffic = traffic;
        _commands = commands;
        _floatingVisible = floatingVisible;
        _toggleFlyout = toggleFlyout;

        using var stream = Application.GetResourceStream(new Uri("pack://application:,,,/Anemobit;component/Assets/Anemobit.ico")).Stream;
        _image = new System.Drawing.Icon(stream, Forms.SystemInformation.SmallIconSize);
        _icon = new Forms.NotifyIcon { Icon = _image, Text = traffic.TrayToolTip, Visible = true };
        _icon.MouseClick += OnMouseClick;
        _traffic.PropertyChanged += OnTrafficChanged;
    }

    /// <summary>First-launch tip: Windows 11 puts new icons in the "^" overflow by default.</summary>
    public void ShowFirstRunHint() =>
        _icon.ShowBalloonTip(10_000, Strings.TrayFirstRunTitle, Strings.TrayFirstRunMessage, Forms.ToolTipIcon.Info);

    public void Dispose()
    {
        _traffic.PropertyChanged -= OnTrafficChanged;
        _icon.Visible = false;
        _icon.Dispose();
        _image.Dispose();
        _menuHost?.Close();
    }

    private void OnTrafficChanged(object? sender, PropertyChangedEventArgs e)
    {
        if (e.PropertyName == nameof(TrafficViewModel.TrayToolTip))
        {
            _icon.Text = _traffic.TrayToolTip;
        }
    }

    private void OnMouseClick(object? sender, Forms.MouseEventArgs e)
    {
        if (e.Button == Forms.MouseButtons.Left)
        {
            _toggleFlyout(Forms.Cursor.Position);
        }
        else if (e.Button == Forms.MouseButtons.Right)
        {
            ShowMenu();
        }
    }

    /// <summary>
    /// WPF menus need a foreground window to close when clicking outside them; an invisible tiny window serves as the host.
    /// </summary>
    private void ShowMenu()
    {
        _menuHost ??= CreateMenuHost();
        _menuHost.Show();
        _menuHost.Activate();
        NativeMethods.BringToForeground(_menuHost);

        var menu = AppMenu.Create(_commands, _floatingVisible());
        menu.Placement = System.Windows.Controls.Primitives.PlacementMode.MousePoint;
        menu.PlacementTarget = _menuHost;
        menu.Closed += (_, _) => _menuHost.Hide();
        menu.IsOpen = true;
    }

    private static Window CreateMenuHost()
    {
        var host = new Window
        {
            Width = 1,
            Height = 1,
            Left = -100,
            Top = -100,
            WindowStyle = WindowStyle.None,
            ResizeMode = ResizeMode.NoResize,
            ShowInTaskbar = false,
            AllowsTransparency = true,
            Background = System.Windows.Media.Brushes.Transparent,
            Opacity = 0,
        };
        host.SourceInitialized += (_, _) => NativeMethods.MakeToolWindow(host, noActivate: false);
        return host;
    }
}
