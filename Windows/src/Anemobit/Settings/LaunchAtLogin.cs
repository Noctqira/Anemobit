using Microsoft.Win32;

namespace Anemobit.Settings;

/// <summary>
/// "Open at Login": writes to <c>HKCU\Software\Microsoft\Windows\CurrentVersion\Run</c>.
/// The system is the source of truth: the user may disable it under Startup apps in Task Manager (recorded in <c>Explorer\StartupApproved\Run</c>),
/// so it's read again every time; turning it on here also clears Task Manager's disabled state.
/// </summary>
/// <param name="root">Registry root key (HKCU in production; a temporary key in tests).</param>
/// <param name="command">Command line run at login (including <see cref="StartupArgument"/>).</param>
public sealed class LaunchAtLogin(RegistryKey root, string command)
{
    /// <summary>Argument for launching at login: the app then doesn't open the main window on its own.</summary>
    public const string StartupArgument = "--startup";

    private const string RunPath = @"Software\Microsoft\Windows\CurrentVersion\Run";
    private const string ApprovedPath = @"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";
    private const string ValueName = "Anemobit";

    /// <summary>StartupApproved value: a first byte of 2 (or even) means enabled; 3 (odd) means disabled by the user.</summary>
    private static readonly byte[] ApprovedEnabled = [0x02, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

    public static LaunchAtLogin ForCurrentUser() =>
        new(Registry.CurrentUser, $"\"{Environment.ProcessPath}\" {StartupArgument}");

    public bool IsEnabled
    {
        get
        {
            using var run = root.OpenSubKey(RunPath);
            if (run?.GetValue(ValueName) is not string)
            {
                return false;
            }
            using var approved = root.OpenSubKey(ApprovedPath);
            return approved?.GetValue(ValueName) is not byte[] { Length: > 0 } state || (state[0] & 1) == 0;
        }
    }

    /// <summary>Applies immediately.</summary>
    /// <exception cref="UnauthorizedAccessException">The registry key can't be written.</exception>
    /// <exception cref="System.Security.SecurityException">The registry key can't be written.</exception>
    public void SetEnabled(bool enabled)
    {
        using var run = root.CreateSubKey(RunPath);
        if (enabled)
        {
            run.SetValue(ValueName, command, RegistryValueKind.String);
            using var approved = root.OpenSubKey(ApprovedPath, writable: true);
            if (approved?.GetValue(ValueName) is not null)
            {
                approved.SetValue(ValueName, ApprovedEnabled, RegistryValueKind.Binary);
            }
        }
        else
        {
            run.DeleteValue(ValueName, throwOnMissingValue: false);
        }
    }

    /// <summary>On app removal: also deletes Task Manager's enabled / disabled record.</summary>
    public void Remove()
    {
        SetEnabled(false);
        using var approved = root.OpenSubKey(ApprovedPath, writable: true);
        approved?.DeleteValue(ValueName, throwOnMissingValue: false);
    }
}
