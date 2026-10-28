namespace Anemobit.Core.Snmp.V3;

/// <summary>A message the client received and decoded: a regular response, or an SNMPv3 Report.</summary>
internal abstract record SnmpIncoming
{
    private SnmpIncoming()
    {
    }

    public sealed record Response(Pdu Pdu) : SnmpIncoming;

    public sealed record Report(Oid? Oid, UsmSecurityParameters Security) : SnmpIncoming;
}
