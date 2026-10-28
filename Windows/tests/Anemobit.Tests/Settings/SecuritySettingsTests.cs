using Anemobit.Core.Snmp;
using Anemobit.Settings;

namespace Anemobit.Tests.Settings;

public class SecuritySettingsTests
{
    [Fact]
    public void ToSnmpSecurity_None_IsCommunity()
    {
        var security = new SecuritySettings { Mode = SecurityMode.None, Community = "public" };
        Assert.Equal(new SnmpSecurity.Community("public"), security.ToSnmpSecurity());
    }

    [Fact]
    public void ToSnmpSecurity_Authorized_IsUsmWithoutPrivacy()
    {
        var security = new SecuritySettings { Mode = SecurityMode.Authorized, Community = "user", AuthProtocol = SnmpAuthProtocol.Sha512, AuthPassword = "authpass12" };
        var usm = Assert.IsType<SnmpSecurity.Usm>(security.ToSnmpSecurity());
        Assert.Equal(new UsmCredentials("user", SnmpAuthProtocol.Sha512, "authpass12"), usm.Credentials);
    }

    [Fact]
    public void ToSnmpSecurity_Private_IncludesPrivacy()
    {
        var security = new SecuritySettings
        {
            Mode = SecurityMode.Private,
            Community = "user",
            AuthPassword = "authpass12",
            PrivProtocol = SnmpPrivProtocol.Des,
            PrivPassword = "privpass12",
        };
        var usm = Assert.IsType<SnmpSecurity.Usm>(security.ToSnmpSecurity());
        Assert.Equal(new UsmPrivacy(SnmpPrivProtocol.Des, "privpass12"), usm.Credentials.Privacy);
    }

    [Theory]
    [InlineData(SecurityMode.None, "", "", "", SecurityProblem.MissingCommunity)]
    [InlineData(SecurityMode.Authorized, "", "authpass12", "", SecurityProblem.MissingUserName)]
    [InlineData(SecurityMode.Authorized, "user", "short", "", SecurityProblem.AuthPasswordTooShort)]
    [InlineData(SecurityMode.Private, "user", "authpass12", "short", SecurityProblem.PrivPasswordTooShort)]
    [InlineData(SecurityMode.None, "public", "", "", null)]
    [InlineData(SecurityMode.Private, "user", "authpass12", "privpass12", null)]
    public void Problem_Fields_ReportsFirstProblem(SecurityMode mode, string community, string auth, string priv, SecurityProblem? expected)
    {
        var security = new SecuritySettings { Mode = mode, Community = community, AuthPassword = auth, PrivPassword = priv };
        Assert.Equal(expected, security.Problem);
    }

    [Fact]
    public void ToString_Secrets_AreNotIncluded()
    {
        var text = new SecuritySettings { Community = "community-secret", AuthPassword = "auth-secret", PrivPassword = "priv-secret" }.ToString();
        Assert.DoesNotContain("secret", text, StringComparison.Ordinal);
    }
}
