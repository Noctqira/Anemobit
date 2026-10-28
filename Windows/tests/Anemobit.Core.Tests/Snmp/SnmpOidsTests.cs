using Anemobit.Core.Snmp;

namespace Anemobit.Core.Tests.Snmp;

public class SnmpOidsTests
{
    [Theory]
    [InlineData(nameof(SnmpOids.IfName), "1.3.6.1.2.1.31.1.1.1.1")]
    [InlineData(nameof(SnmpOids.IfDescr), "1.3.6.1.2.1.2.2.1.2")]
    [InlineData(nameof(SnmpOids.IfHCInOctets), "1.3.6.1.2.1.31.1.1.1.6")]
    [InlineData(nameof(SnmpOids.IfHCOutOctets), "1.3.6.1.2.1.31.1.1.1.10")]
    [InlineData(nameof(SnmpOids.SysUpTime), "1.3.6.1.2.1.1.3.0")]
    public void Field_MatchesSpecification(string name, string expected)
    {
        var oid = (Oid)typeof(SnmpOids).GetField(name)!.GetValue(null)!;
        Assert.Equal(expected, oid.ToString());
    }
}
