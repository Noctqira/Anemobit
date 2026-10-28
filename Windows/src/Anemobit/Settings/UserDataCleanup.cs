using System.IO;

namespace Anemobit.Settings;

/// <summary>
/// Removes the current user's data when Anemobit is uninstalled: the community and passwords in Credential Manager, Open at Login, and the settings folder.
/// Run by the installer on uninstall (not upgrade) as <c>Anemobit.exe --uninstall-cleanup</c>.
/// Each item is handled separately; one failing doesn't affect the others.
/// </summary>
public sealed class UserDataCleanup(ICredentialStore credentials, LaunchAtLogin launchAtLogin, string settingsDirectory)
{
    public const string Argument = "--uninstall-cleanup";

    public static UserDataCleanup ForCurrentUser() => new(
        new WindowsCredentialStore(),
        LaunchAtLogin.ForCurrentUser(),
        Path.GetDirectoryName(SettingsStore.DefaultPath)!);

    public void Run()
    {
        foreach (var name in (ReadOnlySpan<string>)[SettingsStore.CommunityName, SettingsStore.AuthPasswordName, SettingsStore.PrivPasswordName])
        {
            credentials.Delete(name);
        }

        try
        {
            launchAtLogin.Remove();
        }
        catch (Exception error) when (error is UnauthorizedAccessException or System.Security.SecurityException or IOException)
        {
            // Skip if the registry can't be written and continue with the other items.
        }

        try
        {
            if (Directory.Exists(settingsDirectory))
            {
                Directory.Delete(settingsDirectory, recursive: true);
            }
        }
        catch (Exception error) when (error is UnauthorizedAccessException or IOException)
        {
            // Skip files in use (Windows won't abort the uninstall over this).
        }
    }
}
