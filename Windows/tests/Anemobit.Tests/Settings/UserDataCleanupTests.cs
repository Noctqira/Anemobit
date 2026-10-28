using System.IO;
using Microsoft.Win32;
using Anemobit.Settings;
using Anemobit.Tests.Fakes;

namespace Anemobit.Tests.Settings;

/// <summary>Uninstall cleanup: works on a temporary folder, an in-memory credential store, and a temporary key under HKCU\Software\Anemobit.Tests.</summary>
public sealed class UserDataCleanupTests : IDisposable
{
    private const string RunPath = @"Software\Microsoft\Windows\CurrentVersion\Run";
    private const string ApprovedPath = @"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";

    private readonly TemporaryDirectory _directory = new();
    private readonly InMemoryCredentialStore _credentials = new();
    private readonly string _rootPath = @"Software\Anemobit.Tests\" + Guid.NewGuid().ToString("N");
    private readonly RegistryKey _root;

    public UserDataCleanupTests()
    {
        _root = Registry.CurrentUser.CreateSubKey(_rootPath);
    }

    public void Dispose()
    {
        _root.Dispose();
        Registry.CurrentUser.DeleteSubKeyTree(_rootPath, throwOnMissingSubKey: false);
        _directory.Dispose();
    }

    [Fact]
    public void Run_EverythingPresent_RemovesSecretsStartupAndSettings()
    {
        _credentials.Secrets[SettingsStore.CommunityName] = "public";
        _credentials.Secrets[SettingsStore.AuthPasswordName] = "authpass12";
        _credentials.Secrets[SettingsStore.PrivPasswordName] = "privpass12";
        var launch = new LaunchAtLogin(_root, "\"C:\\Anemobit.exe\" --startup");
        launch.SetEnabled(true);
        using (var approved = _root.CreateSubKey(ApprovedPath))
        {
            approved.SetValue("Anemobit", new byte[] { 0x03, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0 }, RegistryValueKind.Binary);
        }
        File.WriteAllText(_directory.SettingsFile, "{}");

        Cleanup(launch).Run();

        Assert.Empty(_credentials.Secrets);
        Assert.False(launch.IsEnabled);
        using (var run = _root.OpenSubKey(RunPath))
        {
            Assert.Null(run?.GetValue("Anemobit"));
        }
        using (var approved = _root.OpenSubKey(ApprovedPath))
        {
            Assert.Null(approved?.GetValue("Anemobit"));
        }
        Assert.False(Directory.Exists(_directory.Path));
    }

    [Fact]
    public void Run_NothingPresent_DoesNotThrow()
    {
        Directory.Delete(_directory.Path, recursive: true);

        Cleanup(new LaunchAtLogin(_root, "x")).Run();

        Assert.Empty(_credentials.Secrets);
    }

    [Fact]
    public void Run_SettingsFileInUse_StillRemovesSecretsAndStartup()
    {
        _credentials.Secrets[SettingsStore.CommunityName] = "public";
        var launch = new LaunchAtLogin(_root, "x");
        launch.SetEnabled(true);
        using var locked = new FileStream(_directory.SettingsFile, FileMode.Create, FileAccess.ReadWrite, FileShare.None);

        Cleanup(launch).Run();

        Assert.Empty(_credentials.Secrets);
        Assert.False(launch.IsEnabled);
    }

    private UserDataCleanup Cleanup(LaunchAtLogin launch) => new(_credentials, launch, _directory.Path);
}
