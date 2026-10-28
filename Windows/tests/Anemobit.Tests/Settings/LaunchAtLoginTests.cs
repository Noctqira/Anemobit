using Microsoft.Win32;
using Anemobit.Settings;

namespace Anemobit.Tests.Settings;

/// <summary>Works on a temporary key under HKCU\Software\Anemobit.Tests, never touching the real startup entries.</summary>
public sealed class LaunchAtLoginTests : IDisposable
{
    private const string RunPath = @"Software\Microsoft\Windows\CurrentVersion\Run";
    private const string ApprovedPath = @"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";
    private const string Command = "\"C:\\Apps\\Anemobit.exe\" --startup";

    private readonly string _rootPath = @"Software\Anemobit.Tests\" + Guid.NewGuid().ToString("N");
    private readonly RegistryKey _root;

    public LaunchAtLoginTests()
    {
        _root = Registry.CurrentUser.CreateSubKey(_rootPath);
    }

    public void Dispose()
    {
        _root.Dispose();
        Registry.CurrentUser.DeleteSubKeyTree(_rootPath, throwOnMissingSubKey: false);
    }

    [Fact]
    public void IsEnabled_NothingRegistered_IsFalse()
    {
        Assert.False(Launch().IsEnabled);
    }

    [Fact]
    public void SetEnabled_True_WritesRunValue()
    {
        Launch().SetEnabled(true);

        using var run = _root.OpenSubKey(RunPath);
        Assert.Equal(Command, run?.GetValue("Anemobit"));
        Assert.True(Launch().IsEnabled);
    }

    [Fact]
    public void SetEnabled_False_RemovesRunValue()
    {
        Launch().SetEnabled(true);

        Launch().SetEnabled(false);

        using var run = _root.OpenSubKey(RunPath);
        Assert.Null(run?.GetValue("Anemobit"));
        Assert.False(Launch().IsEnabled);
    }

    [Fact]
    public void IsEnabled_DisabledInTaskManager_IsFalse()
    {
        Launch().SetEnabled(true);
        using (var approved = _root.CreateSubKey(ApprovedPath))
        {
            approved.SetValue("Anemobit", new byte[] { 0x03, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0 }, RegistryValueKind.Binary);
        }

        Assert.False(Launch().IsEnabled);
    }

    [Fact]
    public void SetEnabled_TrueAfterTaskManagerDisable_ReEnables()
    {
        Launch().SetEnabled(true);
        using (var approved = _root.CreateSubKey(ApprovedPath))
        {
            approved.SetValue("Anemobit", new byte[] { 0x03, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0 }, RegistryValueKind.Binary);
        }

        Launch().SetEnabled(true);

        Assert.True(Launch().IsEnabled);
    }

    private LaunchAtLogin Launch() => new(_root, Command);
}
