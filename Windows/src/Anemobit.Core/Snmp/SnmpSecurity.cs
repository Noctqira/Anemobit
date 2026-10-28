namespace Anemobit.Core.Snmp;

/// <summary>SNMP security settings: v2c community, or SNMPv3 USM. <see cref="object.ToString"/> excludes sensitive data.</summary>
public abstract record SnmpSecurity
{
    private SnmpSecurity()
    {
    }

    /// <summary>SNMPv2c (RouterOS's none).</summary>
    public sealed record Community(string Name) : SnmpSecurity
    {
        public override string ToString() => "Community { }";
    }

    /// <summary>SNMPv3 USM (RouterOS's authorized / private).</summary>
    public sealed record Usm(UsmCredentials Credentials) : SnmpSecurity
    {
        public override string ToString() => $"Usm {{ {Credentials} }}";
    }
}
