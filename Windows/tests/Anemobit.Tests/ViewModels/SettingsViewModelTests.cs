using System.IO;
using Anemobit.Core.Tests.Fakes;
using Anemobit.Core.Transport;
using Anemobit.Settings;
using Anemobit.Tests.Fakes;
using Anemobit.ViewModels;

namespace Anemobit.Tests.ViewModels;

public sealed class SettingsViewModelTests : IDisposable
{
    private readonly TemporaryDirectory _directory = new();
    private readonly InMemoryCredentialStore _credentials = new();
    private readonly SettingsStore _store;
    private IUdpTransport _transport = new FakeTransport();

    public SettingsViewModelTests()
    {
        _store = new SettingsStore(_directory.SettingsFile, _credentials);
    }

    public void Dispose() => _directory.Dispose();

    [Fact]
    public void Constructor_SavedSettings_LoadsDraft()
    {
        _store.Save(new AppSettings
        {
            Host = "router.lan",
            Port = 1161,
            InterfaceNames = ["pppoe-out1"],
            Security = new SecuritySettings { Mode = SecurityMode.Authorized, Community = "user", AuthPassword = "authpass12" },
        });

        using var model = Model();

        Assert.Equal("router.lan", model.Host);
        Assert.Equal("1161", model.PortText);
        Assert.Equal(SecurityMode.Authorized, model.Mode);
        Assert.Equal("user", model.Community);
        Assert.Equal(["pppoe-out1"], model.InterfaceChoices.Select(c => c.Name));
        Assert.True(model.InterfaceChoices[0].IsSelected);
        Assert.Equal("pppoe-out1", model.FloatingInterface);
    }

    [Theory]
    [InlineData("", "161", "public", "Enter the device address")]
    [InlineData("999.1.1.1", "161", "public", "Enter a valid IPv4 or IPv6 address, or a host name")]
    [InlineData("192.168.88.1", "0", "public", "Port must be 1–65535")]
    [InlineData("192.168.88.1", "", "public", "Port must be 1–65535")]
    [InlineData("192.168.88.1", "161", "", "Enter a community")]
    [InlineData("192.168.88.1", "161", "public", null)]
    [InlineData("[2001:db8::1]", "161", "public", null)]
    public void ConnectionProblem_Draft_DescribesFirstProblem(string host, string port, string community, string? expected)
    {
        using var culture = UiCulture.English();
        using var model = Model();

        model.Host = host;
        model.PortText = port;
        model.Community = community;

        Assert.Equal(expected, model.ConnectionProblem);
        Assert.Equal(expected is null, model.CanConnect);
    }

    [Fact]
    public void ConnectionProblem_ShortPrivPassword_IsReported()
    {
        using var culture = UiCulture.English();
        using var model = Model();
        model.Host = "192.168.88.1";
        model.Mode = SecurityMode.Private;
        model.Community = "user";
        model.AuthPassword = "authpass12";
        model.PrivPassword = "short";

        Assert.Equal("The privacy password must be at least 8 characters", model.ConnectionProblem);
        Assert.True(model.IsPrivateMode);
    }

    [Theory]
    [InlineData("１９２．１６８．８８．１", "192.168.88.1")]
    [InlineData("192。168。88。1", "192.168.88.1")]
    [InlineData("router lan", "routerlan")]
    public void Host_TypedText_IsFiltered(string typed, string expected)
    {
        using var model = Model();
        model.Host = typed;
        Assert.Equal(expected, model.Host);
    }

    [Fact]
    public void PortText_TypedText_KeepsDigitsOnly()
    {
        using var model = Model();
        model.PortText = "１６a1";
        Assert.Equal("161", model.PortText);
    }

    [Fact]
    public void CanSave_NoInterfaceSelected_IsFalse()
    {
        using var model = ReadyModel();
        Assert.True(model.CanConnect);
        Assert.False(model.CanSave);
        Assert.False(model.SaveCommand.CanExecute(null));
    }

    [Fact]
    public async Task ScanAsync_RouterOS_ListsInterfacesAndSelectsWan()
    {
        using var culture = UiCulture.English();
        _transport = FakeTransport.ForAgent(new FakeAgent(FakeAgent.RouterOSInterfaceMib()));
        using var model = ReadyModel();

        await model.ScanAsync();

        Assert.Equal(["ether1", "ether2", "bridge", "pppoe-out1", "wlan1"], model.InterfaceChoices.Select(c => c.Name));
        Assert.Equal(["pppoe-out1"], model.SelectedInterfaces);
        Assert.Equal(ActivityKind.Success, model.ActivityKind);
        Assert.Equal("Interfaces found: 5", model.ActivityText);
        Assert.True(model.CanSave);
    }

    [Fact]
    public async Task ScanAsync_NoResponse_ReportsTimeout()
    {
        using var culture = UiCulture.English();
        using var model = ReadyModel();

        await model.ScanAsync();

        Assert.Equal(ActivityKind.Failure, model.ActivityKind);
        Assert.Equal("Connection timed out", model.ActivityText);
        Assert.False(model.IsWorking);
    }

    [Fact]
    public async Task TestAsync_SelectedInterfaces_ReportsUptimeAndIndexes()
    {
        using var culture = UiCulture.English();
        _transport = FakeTransport.ForAgent(new FakeAgent(FakeAgent.RouterOSInterfaceMib()));
        using var model = ReadyModel();
        await model.ScanAsync();
        _transport = FakeTransport.ForAgent(new FakeAgent(FakeAgent.RouterOSInterfaceMib()));

        await model.TestAsync();

        Assert.Equal(ActivityKind.Success, model.ActivityKind);
        Assert.Equal("Connected. Device up 0 minutes. pppoe-out1 is ifIndex 7.", model.ActivityText);
    }

    [Fact]
    public async Task SetSelected_Several_KeepsListOrderAndFloatingFollows()
    {
        _transport = FakeTransport.ForAgent(new FakeAgent(FakeAgent.RouterOSInterfaceMib()));
        using var model = ReadyModel();
        await model.ScanAsync();

        model.SetSelected("wlan1", true);
        model.SetSelected("ether1", true);

        Assert.Equal(["ether1", "pppoe-out1", "wlan1"], model.SelectedInterfaces);
        Assert.True(model.HasMultipleSelected);
        Assert.Equal("pppoe-out1", model.FloatingInterface);

        model.SetSelected("pppoe-out1", false);

        Assert.Equal(["ether1", "wlan1"], model.SelectedInterfaces);
        Assert.Equal("ether1", model.FloatingInterface);
        Assert.False(model.InterfaceChoices.Single(c => c.Name == "pppoe-out1").IsSelected);
    }

    [Fact]
    public void InterfaceChoice_Toggled_UpdatesSelection()
    {
        _store.Save(new AppSettings
        {
            Host = "192.168.88.1",
            InterfaceNames = ["pppoe-out1"],
            Security = new SecuritySettings { Community = "public" },
        });
        using var model = Model();

        model.InterfaceChoices[0].IsSelected = false;

        Assert.Empty(model.SelectedInterfaces);
        Assert.False(model.CanSave);
    }

    [Fact]
    public async Task Save_ValidDraft_WritesSettingsAndRaisesSaved()
    {
        using var culture = UiCulture.English();
        _transport = FakeTransport.ForAgent(new FakeAgent(FakeAgent.RouterOSInterfaceMib()));
        using var model = ReadyModel();
        model.Host = "[2001:db8::1]";
        await model.ScanAsync();
        model.RefreshSeconds = 5;
        model.FloatingOpacityPercent = 40;
        model.DownloadColor = "#abcdef";
        var saved = false;
        model.Saved += (_, _) => saved = true;

        model.Save();

        Assert.True(saved);
        Assert.Equal("Saved", model.ActivityText);
        var settings = _store.Current;
        Assert.Equal("2001:db8::1", settings.Host);
        Assert.Equal(["pppoe-out1"], settings.InterfaceNames);
        Assert.Equal(5, settings.RefreshSeconds);
        Assert.Equal(0.4, settings.FloatingOpacity);
        Assert.Equal("#ABCDEF", settings.DownloadColor);
        Assert.Equal("public", _credentials.Secrets[SettingsStore.CommunityName]);
    }

    [Fact]
    public async Task Save_CredentialStoreFails_ReportsFailure()
    {
        using var culture = UiCulture.English();
        _transport = FakeTransport.ForAgent(new FakeAgent(FakeAgent.RouterOSInterfaceMib()));
        using var model = ReadyModel();
        await model.ScanAsync();
        _credentials.FailWrites = true;

        model.Save();

        Assert.Equal(ActivityKind.Failure, model.ActivityKind);
        Assert.Equal("Could not save the community or passwords to Windows Credential Manager", model.ActivityText);
    }

    [Fact]
    public void ResetColorCommands_CustomColors_ReturnToDefault()
    {
        using var model = Model();
        model.DownloadColor = "#123456";
        model.UploadColor = "#654321";

        model.ResetDownloadColorCommand.Execute(null);
        model.ResetUploadColorCommand.Execute(null);

        Assert.Null(model.DownloadColor);
        Assert.False(model.HasCustomUploadColor);
    }

    [Fact]
    public void OpenAtLogin_NoRegistration_IsFalseAndIgnoresChanges()
    {
        using var model = Model();
        model.OpenAtLogin = true;
        Assert.False(model.OpenAtLogin);
    }

    [Fact]
    public void Language_Initially_ReflectsStoreAndNeedsNoRestart()
    {
        _store.Update(s => s with { Language = AppLanguage.Korean });

        using var model = Model(launchLanguage: AppLanguage.Korean);

        Assert.Equal(AppLanguage.Korean, model.Language);
        Assert.False(model.NeedsRestart);
    }

    [Fact]
    public void Language_Changed_WritesImmediatelyWithoutSave()
    {
        using var model = Model();

        model.Language = AppLanguage.Japanese;

        Assert.Equal(AppLanguage.Japanese, _store.Current.Language);
        Assert.Equal(AppLanguage.Japanese, new SettingsStore(_directory.SettingsFile, _credentials).Current.Language);
        Assert.Equal(string.Empty, _store.Current.Host); // The rest of the draft isn't saved
    }

    [Fact]
    public void NeedsRestart_DiffersFromLaunchLanguage_UntilChangedBack()
    {
        using var model = Model();

        model.Language = AppLanguage.TraditionalChinese;
        Assert.True(model.NeedsRestart);

        model.Language = AppLanguage.System;
        Assert.False(model.NeedsRestart);
    }

    [Fact]
    public void RestartCommand_Executed_InvokesRestart()
    {
        var restarts = 0;
        using var model = Model(restart: () => restarts++);

        model.Language = AppLanguage.English;
        model.RestartCommand.Execute(null);

        Assert.Equal(1, restarts);
    }

    [Fact]
    public void Language_WriteFails_ShowsErrorAndKeepsStoredLanguage()
    {
        using var model = Model();
        Directory.CreateDirectory(_directory.SettingsFile + ".tmp"); // The temporary file can't be created

        model.Language = AppLanguage.Japanese;

        Assert.Equal(AppLanguage.System, model.Language);
        Assert.Equal(AppLanguage.System, _store.Current.Language);
        Assert.NotNull(model.LanguageError);
        Assert.False(model.NeedsRestart);
    }

    [Fact]
    public void Save_AfterLanguageChange_KeepsLanguage()
    {
        using var model = ReadyModel();
        model.Language = AppLanguage.Korean;
        model.SetSelected("ether1", true);

        model.Save();

        Assert.Equal(AppLanguage.Korean, _store.Current.Language);
    }

    private SettingsViewModel Model(AppLanguage launchLanguage = AppLanguage.System, Action? restart = null) =>
        new(_store, (_, _, _) => Task.FromResult(_transport), launchAtLogin: null, launchLanguage, restart, timeout: TimeSpan.FromMilliseconds(50));

    /// <summary>Draft with the address and community filled in and no interfaces selected yet; a short timeout makes "no response" tests finish quickly.</summary>
    private SettingsViewModel ReadyModel()
    {
        var model = Model();
        model.Host = "192.168.88.1";
        model.Community = "public";
        return model;
    }
}
