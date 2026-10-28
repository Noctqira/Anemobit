using Anemobit.Core.Snmp;
using Anemobit.Tests.Fakes;
using Anemobit.Text;

namespace Anemobit.Tests.Text;

public class ErrorTextTests
{
    public static TheoryData<SnmpException, string, string> Errors => new()
    {
        { SnmpException.Timeout(), "Connection timed out", "連線逾時" },
        { SnmpException.InterfaceNotFound("pppoe-out1"), "Interface pppoe-out1 not found", "找不到介面 pppoe-out1" },
        { SnmpException.FromErrorStatus(2, 1), "SNMP error NoSuchName (varbind 1)", "SNMP 錯誤 NoSuchName（varbind 1）" },
        { SnmpException.Authentication(UsmFailure.WrongDigest), "Wrong authentication password or protocol", "認證密碼或認證協定錯誤" },
        { SnmpException.Authentication(UsmFailure.DecryptionError), "Wrong privacy password or protocol", "加密密碼或加密協定錯誤" },
        { SnmpException.Authentication(UsmFailure.UnknownUserName), "Unknown user name", "使用者名稱錯誤" },
        { SnmpException.Authentication(UsmFailure.PrivPasswordTooShort), "The privacy password must be at least 8 characters", "加密密碼至少需要 8 個字元" },
        { SnmpException.UnexpectedResponse("OID not increasing: 1.3.6"), "Unexpected response: OID not increasing: 1.3.6", "非預期的回應：OID not increasing: 1.3.6" },
        { SnmpException.UnsupportedVersion(0), "Unsupported SNMP version 0", "不支援的 SNMP 版本 0" },
        { SnmpException.Transport("host unreachable"), "Connection error: host unreachable", "連線錯誤：host unreachable" },
    };

    [Theory]
    [MemberData(nameof(Errors))]
    public void Describe_SnmpException_IsLocalized(SnmpException error, string english, string chinese)
    {
        using (UiCulture.English())
        {
            Assert.Equal(english, ErrorText.Describe(error));
        }
        using (new UiCulture("zh-TW"))
        {
            Assert.Equal(chinese, ErrorText.Describe(error));
        }
    }

    [Fact]
    public void Describe_OtherException_IsConnectionError()
    {
        using (UiCulture.English())
        {
            Assert.Equal("Connection error: cannot resolve", ErrorText.Describe(new InvalidOperationException("cannot resolve")));
        }
    }
}
