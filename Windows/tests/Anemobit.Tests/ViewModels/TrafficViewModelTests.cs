using Anemobit.Core.Formatting;
using Anemobit.Core.Snmp;
using Anemobit.Core.Traffic;
using Anemobit.Settings;
using Anemobit.Tests.Fakes;
using Anemobit.ViewModels;

namespace Anemobit.Tests.ViewModels;

public sealed class TrafficViewModelTests : IDisposable
{
    private readonly TemporaryDirectory _directory = new();
    private readonly List<FakePoller> _pollers = [];
    private readonly SettingsStore _store;

    public TrafficViewModelTests()
    {
        _store = new SettingsStore(_directory.SettingsFile, new InMemoryCredentialStore());
    }

    public void Dispose() => _directory.Dispose();

    [Fact]
    public void Start_NotConfigured_ShowsNotSetWithoutPolling()
    {
        using var culture = UiCulture.English();
        using var model = Model();

        model.Start();

        Assert.IsType<ConnectionState.NotConfigured>(model.State);
        Assert.False(model.IsConfigured);
        Assert.Empty(model.Interfaces);
        Assert.Empty(_pollers);
        Assert.Equal("Anemobit: Not Set", model.FloatingText);
        Assert.Equal("Anemobit", model.Title);
    }

    [Fact]
    public void Start_Configured_CreatesCardsAndPollsWithSettings()
    {
        _store.Save(Configured(["pppoe-out1", "ether1"]));
        using var model = Model();

        model.Start();

        Assert.IsType<ConnectionState.Connecting>(model.State);
        Assert.Equal(["pppoe-out1", "ether1"], model.Interfaces.Select(i => i.Name));
        var poller = Assert.Single(_pollers);
        Assert.Equal("192.168.88.1", poller.Configuration.Host);
        Assert.Equal(["pppoe-out1", "ether1"], poller.Configuration.InterfaceNames);
        Assert.Equal(new SnmpSecurity.Community("public"), poller.Configuration.Security);
    }

    [Fact]
    public async Task Updates_ConnectedAndSnapshot_FillInterfacesAndFloatingText()
    {
        using var culture = UiCulture.English();
        _store.Save(Configured(["pppoe-out1", "ether1"]) with { FloatingInterface = "ether1" });
        using var model = Model();
        model.Start();
        var poller = _pollers[0];
        var at = DateTimeOffset.UtcNow;

        poller.Send(new PollerStatus.Connected());
        poller.Send(new TrafficSnapshot(at, TimeSpan.FromSeconds(3_660), new Dictionary<string, TrafficSample>
        {
            ["pppoe-out1"] = new(at, 10_000_000, 1_000_000),
            ["ether1"] = new(at, 2_000_000, 500_000),
        }));
        poller.Complete();
        await model.Completion;

        Assert.IsType<ConnectionState.Connected>(model.State);
        Assert.Equal(StatusKind.Ok, model.StatusKind);
        Assert.Equal("OK", model.StatusText);
        Assert.Equal("10.0 Mbps", model.Interfaces[0].CurrentIn);
        Assert.Equal("ether1", model.FloatingInterface?.Name);
        Assert.Equal("↓ 2.0 Mbps", model.FloatingDownText);
        Assert.Equal("↑ 500.0 Kbps", model.FloatingUpText);
        Assert.Equal("Device up 1 hour, 1 minute", model.DeviceUpText);
        Assert.Equal("192.168.88.1", model.Title);
        Assert.Equal("2 interfaces", model.Subtitle);
        Assert.Contains("ether1", model.TrayToolTip, StringComparison.Ordinal);
    }

    [Fact]
    public async Task Updates_Failure_ShowsErrorAndClearsCurrentRates()
    {
        using var culture = UiCulture.English();
        _store.Save(Configured(["pppoe-out1"]));
        using var model = Model();
        model.Start();
        var poller = _pollers[0];
        var at = DateTimeOffset.UtcNow;

        poller.Send(new PollerStatus.Connected());
        poller.Send(new TrafficSnapshot(at, null, new Dictionary<string, TrafficSample> { ["pppoe-out1"] = new(at, 1_000, 1_000) }));
        poller.Send(new PollerStatus.Failed(SnmpException.Timeout(), TimeSpan.FromSeconds(4)));
        poller.Complete();
        await model.Completion;

        Assert.Equal(StatusKind.Failed, model.StatusKind);
        Assert.Equal("Error · retry in 4 s", model.StatusText);
        Assert.Equal("Connection timed out", model.ErrorMessage);
        Assert.Equal("Connection timed out", model.StatusDetail);
        Assert.Null(model.Interfaces[0].Latest);
        Assert.Single(model.Interfaces[0].History);
        Assert.Equal("↓ --", model.FloatingDownText);
        // Single interface: title is the interface name, subtitle the device
        Assert.Equal("pppoe-out1", model.Title);
        Assert.Equal("192.168.88.1", model.Subtitle);
    }

    [Fact]
    public void SettingsChange_DisplayOnly_KeepsPolling()
    {
        _store.Save(Configured(["pppoe-out1"]));
        using var model = Model();
        model.Start();

        _store.Save(_store.Current with { UnitStyle = RateUnitStyle.Bytes, ChartsWhenMultiple = true, DownloadColor = "#123456" });

        Assert.Single(_pollers);
        Assert.False(_pollers[0].Stopped);
        Assert.Equal(RateUnitStyle.Bytes, model.Interfaces[0].UnitStyle);
    }

    [Fact]
    public async Task SettingsChange_Connection_RestartsPolling()
    {
        _store.Save(Configured(["pppoe-out1"]));
        using var model = Model();
        model.Start();
        var first = model.Completion;

        _store.Save(_store.Current with { InterfaceNames = ["pppoe-out1", "ether1"] });
        await first;

        Assert.Equal(2, _pollers.Count);
        Assert.True(_pollers[0].Stopped);
        Assert.Equal(["pppoe-out1", "ether1"], model.Interfaces.Select(i => i.Name));
    }

    [Fact]
    public async Task SettingsChange_RefreshRate_RestartsPolling()
    {
        _store.Save(Configured(["pppoe-out1"]));
        using var model = Model();
        model.Start();
        var first = model.Completion;

        _store.Save(_store.Current with { RefreshSeconds = 5 });
        await first;

        Assert.Equal(2, _pollers.Count);
        Assert.Equal(TimeSpan.FromSeconds(5), _pollers[1].Configuration.Interval);
    }

    [Fact]
    public void ShowsCharts_MultipleInterfaces_FollowsSetting()
    {
        _store.Save(Configured(["pppoe-out1", "ether1"]));
        using var model = Model();
        model.Start();
        Assert.False(model.ShowsCharts);

        _store.Save(_store.Current with { ChartsWhenMultiple = true });

        Assert.True(model.ShowsCharts);
    }

    [Fact]
    public void ShowsCharts_SingleInterface_AlwaysTrue()
    {
        _store.Save(Configured(["pppoe-out1"]));
        using var model = Model();
        model.Start();

        Assert.True(model.ShowsCharts);
    }

    [Fact]
    public void TrayToolTip_LongInterfaceName_IsLimitedTo127Characters()
    {
        _store.Save(Configured([new string('x', 200)]));
        using var model = Model();
        model.Start();

        Assert.True(model.TrayToolTip.Length <= 127);
    }

    [Fact]
    public async Task Dispose_WhilePolling_StopsPoller()
    {
        _store.Save(Configured(["pppoe-out1"]));
        var model = Model();
        model.Start();

        model.Dispose();
        await model.Completion;

        Assert.True(_pollers[0].Stopped);
    }

    private TrafficViewModel Model() => new(_store, configuration =>
    {
        var poller = new FakePoller(configuration);
        _pollers.Add(poller);
        return poller;
    });

    private static AppSettings Configured(IReadOnlyList<string> interfaces) => new()
    {
        Host = "192.168.88.1",
        InterfaceNames = interfaces,
        Security = new SecuritySettings { Mode = SecurityMode.None, Community = "public" },
    };
}
