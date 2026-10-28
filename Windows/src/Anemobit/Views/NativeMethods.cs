using System.Runtime.InteropServices;
using System.Windows;
using System.Windows.Interop;

namespace Anemobit.Views;

/// <summary>Window settings WPF doesn't provide (Win32).</summary>
internal static partial class NativeMethods
{
    private const int GwlExStyle = -20;
    private const long WsExToolWindow = 0x00000080;
    private const long WsExNoActivate = 0x08000000;
    private const long WsExAppWindow = 0x00040000;
    private const int DwmwaWindowCornerPreference = 33;
    private const int DwmwcpRound = 2;
    private const int DwmwaSystemBackdropType = 38;
    private const int DwmsbtTransientWindow = 3; // Acrylic
    private const uint SwpNoSize = 0x0001;
    private const uint SwpNoZOrder = 0x0004;
    private const uint SwpNoActivate = 0x0010;

    /// <summary>Keeps the window out of the taskbar and Alt+Tab, and doesn't take focus when clicked (floating window).</summary>
    public static void MakeToolWindow(Window window, bool noActivate)
    {
        var handle = new WindowInteropHelper(window).EnsureHandle();
        var style = GetWindowLongPtr(handle, GwlExStyle).ToInt64();
        style = (style | WsExToolWindow) & ~WsExAppWindow;
        if (noActivate)
        {
            style |= WsExNoActivate;
        }
        SetWindowLongPtr(handle, GwlExStyle, new IntPtr(style));
    }

    /// <summary>Windows 11 rounded corners (borderless windows are square by default).</summary>
    public static void UseRoundedCorners(Window window)
    {
        var handle = new WindowInteropHelper(window).EnsureHandle();
        var preference = DwmwcpRound;
        _ = DwmSetWindowAttribute(handle, DwmwaWindowCornerPreference, ref preference, sizeof(int));
    }

    /// <summary>Sets the system backdrop to Acrylic (Windows 11 22H2 and later; older systems ignore it).</summary>
    public static void UseAcrylicBackdrop(Window window)
    {
        var handle = new WindowInteropHelper(window).Handle;
        if (handle == IntPtr.Zero)
        {
            return;
        }
        var backdrop = DwmsbtTransientWindow;
        _ = DwmSetWindowAttribute(handle, DwmwaSystemBackdropType, ref backdrop, sizeof(int));
    }

    /// <summary>Moves the window in physical pixels (unaffected by per-monitor DPI scaling).</summary>
    public static void MoveTo(Window window, int x, int y)
    {
        var handle = new WindowInteropHelper(window).EnsureHandle();
        SetWindowPos(handle, IntPtr.Zero, x, y, 0, 0, SwpNoSize | SwpNoZOrder | SwpNoActivate);
    }

    /// <summary>The window's bounds in physical pixels.</summary>
    public static (int Left, int Top, int Width, int Height) Bounds(Window window)
    {
        var handle = new WindowInteropHelper(window).EnsureHandle();
        GetWindowRect(handle, out var rect);
        return (rect.Left, rect.Top, rect.Right - rect.Left, rect.Bottom - rect.Top);
    }

    public static void BringToForeground(Window window) =>
        SetForegroundWindow(new WindowInteropHelper(window).EnsureHandle());

    [LibraryImport("user32.dll", EntryPoint = "GetWindowLongPtrW")]
    private static partial IntPtr GetWindowLongPtr(IntPtr hWnd, int index);

    [LibraryImport("user32.dll", EntryPoint = "SetWindowLongPtrW")]
    private static partial IntPtr SetWindowLongPtr(IntPtr hWnd, int index, IntPtr newLong);

    [LibraryImport("dwmapi.dll")]
    private static partial int DwmSetWindowAttribute(IntPtr hwnd, int attribute, ref int value, int size);

    [LibraryImport("user32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static partial bool SetWindowPos(IntPtr hWnd, IntPtr insertAfter, int x, int y, int width, int height, uint flags);

    [LibraryImport("user32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static partial bool GetWindowRect(IntPtr hWnd, out NativeRect rect);

    [LibraryImport("user32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static partial bool SetForegroundWindow(IntPtr hWnd);

    [StructLayout(LayoutKind.Sequential)]
    private struct NativeRect
    {
        public int Left;
        public int Top;
        public int Right;
        public int Bottom;
    }
}
