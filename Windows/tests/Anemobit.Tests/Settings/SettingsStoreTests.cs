using System.IO;
using Anemobit.Core.Formatting;
using Anemobit.Core.Snmp;
using Anemobit.Settings;
using Anemobit.Tests.Fakes;

namespace Anemobit.Tests.Settings;

public sealed class SettingsStoreTests : IDisposable
{
    private readonly TemporaryDirectory _directory = new();
    private readonly InMemoryCredentialStore _credentials = new();

    public void Dispose() => _directory.Dispose();

    [Fact]
    public void Constructor_NoSettingsFile_UsesDefaults()
    {
        var settings = Store().Current;

        Assert.Equal(string.Empty, settings.Host);
        Assert.Equal(161, settings.Port);
        Assert.Empty(settings.InterfaceNames);
        Assert.Equal(SecurityMode.None, settings.Security.Mode);
        Assert.Equal(SnmpAuthProtocol.Sha1, settings.Security.AuthProtocol);
        Assert.Equal(SnmpPrivProtocol.Aes, settings.Security.PrivProtocol);
        Assert.Equal(1, settings.RefreshSeconds);
        Assert.Equal(RateUnitStyle.Bits, settings.UnitStyle);
        Assert.False(settings.ChartsWhenMultiple);
        Assert.True(settings.ShowFloatingWindow);
        Assert.Equal(0.7, settings.FloatingOpacity);
        Assert.Equal(AppLanguage.System, settings.Language);
        Assert.False(settings.IsConfigured);
    }

    [Fact]
    public void Save_ThenReload_RoundTripsEverything()
    {
        var saved = SampleSettings();
        Store().Save(saved);

        var loaded = Store().Current;

        Assert.Equal(saved.Host, loaded.Host);
        Assert.Equal(saved.Port, loaded.Port);
        Assert.Equal(saved.InterfaceNames, loaded.InterfaceNames);
        Assert.Equal(saved.FloatingInterface, loaded.FloatingInterface);
        Assert.Equal(saved.Security, loaded.Security);
        Assert.Equal(saved.RefreshSeconds, loaded.RefreshSeconds);
        Assert.Equal(saved.UnitStyle, loaded.UnitStyle);
        Assert.Equal(saved.ChartsWhenMultiple, loaded.ChartsWhenMultiple);
        Assert.Equal(saved.DownloadColor, loaded.DownloadColor);
        Assert.Equal(saved.UploadColor, loaded.UploadColor);
        Assert.Equal(saved.ShowFloatingWindow, loaded.ShowFloatingWindow);
        Assert.Equal(saved.FloatingAlwaysOnTop, loaded.FloatingAlwaysOnTop);
        Assert.Equal(saved.FloatingOpacity, loaded.FloatingOpacity);
        Assert.Equal(saved.FloatingPosition, loaded.FloatingPosition);
        Assert.Equal(saved.Language, loaded.Language);
        Assert.True(loaded.IsConfigured);
    }

    [Fact]
    public void Save_Secrets_GoToCredentialStoreNotSettingsFile()
    {
        Store().Save(SampleSettings());

        var json = File.ReadAllText(_directory.SettingsFile);
        Assert.DoesNotContain("my-community", json, StringComparison.Ordinal);
        Assert.DoesNotContain("auth-secret", json, StringComparison.Ordinal);
        Assert.DoesNotContain("priv-secret", json, StringComparison.Ordinal);
        Assert.Equal("my-community", _credentials.Secrets[SettingsStore.CommunityName]);
        Assert.Equal("auth-secret-1", _credentials.Secrets[SettingsStore.AuthPasswordName]);
        Assert.Equal("priv-secret-2", _credentials.Secrets[SettingsStore.PrivPasswordName]);
    }

    [Fact]
    public void Save_SecurityMode_UsesRouterOSNames()
    {
        Store().Save(SampleSettings());

        var json = File.ReadAllText(_directory.SettingsFile);
        Assert.Contains("\"securityMode\": \"private\"", json, StringComparison.Ordinal);
        Assert.Contains("\"authProtocol\": \"sha256\"", json, StringComparison.Ordinal);
        Assert.Contains("\"device\"", json, StringComparison.Ordinal);
    }

    [Fact]
    public void Save_Language_UsesCultureName()
    {
        Store().Save(SampleSettings());

        var json = File.ReadAllText(_directory.SettingsFile);
        Assert.Contains("\"language\": \"zh-Hant\"", json, StringComparison.Ordinal);
    }

    [Fact]
    public void Save_SystemLanguage_WritesNull()
    {
        Store().Save(SampleSettings() with { Language = AppLanguage.System });

        var json = File.ReadAllText(_directory.SettingsFile);
        Assert.Contains("\"language\": null", json, StringComparison.Ordinal);
        Assert.Equal(AppLanguage.System, Store().Current.Language);
    }

    [Fact]
    public void Save_EmptySecret_DeletesCredential()
    {
        _credentials.Secrets[SettingsStore.PrivPasswordName] = "old";

        Store().Save(SampleSettings() with { Security = SampleSettings().Security with { PrivPassword = string.Empty } });

        Assert.False(_credentials.Secrets.ContainsKey(SettingsStore.PrivPasswordName));
    }

    [Fact]
    public void Save_CredentialWriteFails_ThrowsAndKeepsCurrent()
    {
        var store = Store();
        var changed = false;
        store.Changed += (_, _) => changed = true;
        _credentials.FailWrites = true;

        var error = Assert.Throws<SettingsSaveException>(() => store.Save(SampleSettings()));

        Assert.Equal(SettingsSaveFailure.Credentials, error.Failure);
        Assert.Equal(string.Empty, store.Current.Host);
        Assert.False(changed);
    }

    [Fact]
    public void Save_Success_RaisesChanged()
    {
        var store = Store();
        var changed = 0;
        store.Changed += (_, _) => changed++;

        store.Save(SampleSettings());

        Assert.Equal(1, changed);
        Assert.Equal("192.168.88.1", store.Current.Host);
    }

    [Fact]
    public void Update_OneField_WritesFileAndRaisesChanged()
    {
        var store = Store();
        store.Save(SampleSettings());
        var changed = 0;
        store.Changed += (_, _) => changed++;

        store.Update(s => s with { ShowFloatingWindow = true });

        Assert.Equal(1, changed);
        Assert.True(Store().Current.ShowFloatingWindow);
        Assert.Equal("192.168.88.1", Store().Current.Host);
    }

    [Fact]
    public void Update_NoChange_DoesNotRaiseChanged()
    {
        var store = Store();
        var changed = 0;
        store.Changed += (_, _) => changed++;

        store.Update(s => s);

        Assert.Equal(0, changed);
    }

    [Fact]
    public void Constructor_CorruptFile_UsesDefaults()
    {
        File.WriteAllText(_directory.SettingsFile, "{ not json");

        Assert.Equal(string.Empty, Store().Current.Host);
    }

    [Fact]
    public void Constructor_InvalidValues_FallBackToDefaults()
    {
        File.WriteAllText(_directory.SettingsFile, """
            {
              "device": { "host": "router.lan", "port": 0, "interfaceNames": ["ether1", "ether1"] },
              "snmp": { "securityMode": "bogus", "authProtocol": "sha224", "privProtocol": "aes256" },
              "display": { "refreshSeconds": 7, "unitStyle": "nibbles", "downloadColor": "red", "language": "fr" },
              "floatingWindow": { "opacity": 3 }
            }
            """);

        var settings = Store().Current;

        Assert.Equal(161, settings.Port);
        Assert.Equal(["ether1"], settings.InterfaceNames);
        Assert.Equal(SecurityMode.None, settings.Security.Mode);
        Assert.Equal(SnmpAuthProtocol.Sha1, settings.Security.AuthProtocol); // The Windows version has no SHA224
        Assert.Equal(SnmpPrivProtocol.Aes, settings.Security.PrivProtocol);
        Assert.Equal(1, settings.RefreshSeconds);
        Assert.Equal(RateUnitStyle.Bits, settings.UnitStyle);
        Assert.Null(settings.DownloadColor);
        Assert.Equal(1, settings.FloatingOpacity);
        Assert.Equal(AppLanguage.System, settings.Language);
    }

    [Fact]
    public void Save_SettingsFolderMissing_CreatesIt()
    {
        var nested = Path.Combine(_directory.Path, "nested", "settings.json");

        new SettingsStore(nested, _credentials).Save(SampleSettings());

        Assert.True(File.Exists(nested));
    }

    private SettingsStore Store() => new(_directory.SettingsFile, _credentials);

    private static AppSettings SampleSettings() => new()
    {
        Host = "192.168.88.1",
        Port = 1161,
        InterfaceNames = ["pppoe-out1", "ether1"],
        FloatingInterface = "ether1",
        Security = new SecuritySettings
        {
            Mode = SecurityMode.Private,
            Community = "my-community",
            AuthProtocol = SnmpAuthProtocol.Sha256,
            AuthPassword = "auth-secret-1",
            PrivProtocol = SnmpPrivProtocol.Des,
            PrivPassword = "priv-secret-2",
        },
        RefreshSeconds = 5,
        UnitStyle = RateUnitStyle.Bytes,
        ChartsWhenMultiple = true,
        DownloadColor = "#123456",
        UploadColor = "#ABCDEF",
        ShowFloatingWindow = false,
        FloatingAlwaysOnTop = false,
        FloatingOpacity = 0.25,
        FloatingPosition = new FloatingWindowPosition(@"\\.\DISPLAY2", 1200, 800),
        Language = AppLanguage.TraditionalChinese,
    };
}
