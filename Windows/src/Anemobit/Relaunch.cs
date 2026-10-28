using System.ComponentModel;
using System.Diagnostics;

namespace Anemobit;

/// <summary>
/// Restarts Anemobit (used after changing the display language, which resources only read at launch).
/// Starts a new process with <see cref="Argument"/>; the new instance waits for this one to exit instead of handing off to it, then opens Settings.
/// </summary>
internal static class Relaunch
{
    public const string Argument = "--relaunch";

    /// <summary>How long a relaunched instance waits for the previous one to exit before falling back to the normal hand-off.</summary>
    public static TimeSpan Timeout { get; } = TimeSpan.FromSeconds(5);

    /// <summary>Starts the new instance; returns false if it couldn't start (the caller keeps running).</summary>
    public static bool TryStart()
    {
        if (Environment.ProcessPath is not { } path)
        {
            return false;
        }
        try
        {
            using var process = Process.Start(new ProcessStartInfo(path, Argument) { UseShellExecute = false });
            return process is not null;
        }
        catch (Win32Exception)
        {
            return false;
        }
    }
}
