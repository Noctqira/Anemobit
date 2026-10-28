namespace Anemobit.Core.Snmp.V3;

/// <summary>msgFlags (RFC 3412).</summary>
[Flags]
internal enum SnmpV3Flags : byte
{
    None = 0,
    Auth = 0x01,
    Priv = 0x02,
    Reportable = 0x04,
}
