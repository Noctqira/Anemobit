namespace Anemobit.Settings;

/// <summary>Floating window position: its monitor (device name) and the top-left corner in screen coordinates (physical pixels).</summary>
public sealed record FloatingWindowPosition(string Monitor, int X, int Y);
