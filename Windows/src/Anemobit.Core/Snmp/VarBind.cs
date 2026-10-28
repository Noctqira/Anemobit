namespace Anemobit.Core.Snmp;

/// <summary>An SNMP OID and value pair.</summary>
public sealed record VarBind(Oid Oid, SnmpValue Value);
