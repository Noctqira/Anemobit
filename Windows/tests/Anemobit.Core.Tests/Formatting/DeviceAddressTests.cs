using Anemobit.Core.Formatting;

namespace Anemobit.Core.Tests.Formatting;

public class DeviceAddressTests
{
    [Theory]
    [InlineData("192.168.88.1", DeviceAddressKind.IPv4, "192.168.88.1")]
    [InlineData(" 10.0.0.254 ", DeviceAddressKind.IPv4, "10.0.0.254")]
    [InlineData("2001:db8::1", DeviceAddressKind.IPv6, "2001:db8::1")]
    [InlineData("[2001:db8::1]", DeviceAddressKind.IPv6, "2001:db8::1")] // Bracket notation
    [InlineData("fd00::1", DeviceAddressKind.IPv6, "fd00::1")]
    [InlineData("::ffff:192.168.88.1", DeviceAddressKind.IPv6, "::ffff:192.168.88.1")]
    [InlineData("router.lan", DeviceAddressKind.HostName, "router.lan")]
    [InlineData("Router.LAN.", DeviceAddressKind.HostName, "Router.LAN")] // Trailing dot
    [InlineData("mikrotik", DeviceAddressKind.HostName, "mikrotik")]
    [InlineData("my-router-1.example.com", DeviceAddressKind.HostName, "my-router-1.example.com")]
    public void TryParse_ValidAddress_ReturnsKindAndHost(string text, DeviceAddressKind kind, string host)
    {
        Assert.True(DeviceAddress.TryParse(text, out var address));
        Assert.Equal(new DeviceAddress(kind, host), address);
    }

    [Theory]
    [InlineData(null)]
    [InlineData("")]
    [InlineData("   ")]
    [InlineData("999.1.1.1")]       // Number above 255
    [InlineData("192.168.88")]      // Only three parts
    [InlineData("192.168.88.1.5")]
    [InlineData("2001:db8:::1")]    // Invalid IPv6
    [InlineData("[192.168.88.1]")]  // Brackets are for IPv6 only
    [InlineData("-router.lan")]     // Labels can't start with -
    [InlineData("router-.lan")]
    [InlineData("router..lan")]
    [InlineData("router_lan")]      // Underscores not allowed
    [InlineData("123.456")]         // All-digit last label (looks like an IP)
    [InlineData("1234.1.1.1")]
    public void TryParse_InvalidAddress_ReturnsFalse(string? text)
    {
        Assert.False(DeviceAddress.TryParse(text, out _));
    }

    [Fact]
    public void TryParse_TooLongHostName_ReturnsFalse()
    {
        Assert.False(DeviceAddress.TryParse(new string('a', 64) + ".lan", out _));
        var name = string.Join('.', Enumerable.Repeat(new string('a', 60), 5));
        Assert.True(name.Length > 253);
        Assert.False(DeviceAddress.TryParse(name, out _));
    }
}
