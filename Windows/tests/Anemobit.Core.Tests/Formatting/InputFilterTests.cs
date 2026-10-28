using Anemobit.Core.Formatting;

namespace Anemobit.Core.Tests.Formatting;

public class InputFilterTests
{
    [Theory]
    [InlineData("192.168.88.1", "192.168.88.1")]
    [InlineData("", "")]
    [InlineData("router.lan", "router.lan")]
    [InlineData("my-router.example.com", "my-router.example.com")]
    [InlineData("2001:db8::1", "2001:db8::1")]
    [InlineData("[2001:db8::1]", "[2001:db8::1]")]
    [InlineData("fe80::1%en0", "fe80::1%en0")]
    [InlineData("192 .168.88.1 ", "192.168.88.1")]         // Whitespace
    [InlineData("１９２．１６８．８８．１", "192.168.88.1")] // Full-width digits and periods (Chinese / Japanese input methods)
    [InlineData("192。168。88。1", "192.168.88.1")]         // CJK full stops
    [InlineData("192｡168｡88｡1", "192.168.88.1")]           // Half-width ideographic full stops
    [InlineData("ㄅ路由器router", "router")]                // Zhuyin and Chinese
    [InlineData("router/admin?x=1", "routeradminx1")]       // Other URL symbols
    public void Host_Text_KeepsOnlyAddressCharacters(string input, string expected)
    {
        Assert.Equal(expected, InputFilter.Host(input));
    }

    [Fact]
    public void Host_TooLong_IsTruncatedToMaximumLength()
    {
        Assert.Equal(InputFilter.HostMaximumLength, InputFilter.Host(new string('a', 300)).Length);
    }

    [Theory]
    [InlineData("161", "161")]
    [InlineData("", "")]
    [InlineData("16a1", "161")]
    [InlineData("１６１", "161")]
    [InlineData("1.6,1", "161")]
    [InlineData("ㄅ1ㄆ6ㄇ1", "161")]
    [InlineData("655350", "65535")] // At most 5 digits
    public void Port_Text_KeepsOnlyDigits(string input, string expected)
    {
        Assert.Equal(expected, InputFilter.Port(input));
    }
}
