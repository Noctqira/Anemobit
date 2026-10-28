using System.Windows;
using System.Windows.Threading;
using Microsoft.Win32;

namespace Anemobit.Views;

/// <summary>
/// Makes the window background translucent Acrylic (blurring what's behind the window), matching the behind-window blur on macOS.
/// The Fluent theme defaults to Mica (wallpaper color only, nothing behind shows through) and reapplies it on load and system theme changes,
/// so this is set again after those.
/// </summary>
internal static class AcrylicBackdrop
{
    public static void Attach(Window window)
    {
        void Apply() => window.Dispatcher.BeginInvoke(() => NativeMethods.UseAcrylicBackdrop(window), DispatcherPriority.ApplicationIdle);
        void OnPreferenceChanged(object? sender, UserPreferenceChangedEventArgs e) => Apply();

        window.SourceInitialized += (_, _) => Apply();
        window.Loaded += (_, _) => Apply();
        window.Activated += (_, _) => Apply();
        SystemEvents.UserPreferenceChanged += OnPreferenceChanged;
        window.Closed += (_, _) => SystemEvents.UserPreferenceChanged -= OnPreferenceChanged;
    }
}
