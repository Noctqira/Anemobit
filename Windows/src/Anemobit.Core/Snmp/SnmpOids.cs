namespace Anemobit.Core.Snmp;

/// <summary>MIB OIDs used by Anemobit.</summary>
public static class SnmpOids
{
    /// <summary>IF-MIB::ifName (table column, followed by ifIndex).</summary>
    public static readonly Oid IfName = new(1, 3, 6, 1, 2, 1, 31, 1, 1, 1, 1);

    /// <summary>IF-MIB::ifDescr.</summary>
    public static readonly Oid IfDescr = new(1, 3, 6, 1, 2, 1, 2, 2, 1, 2);

    /// <summary>IF-MIB::ifHCInOctets (64-bit).</summary>
    public static readonly Oid IfHCInOctets = new(1, 3, 6, 1, 2, 1, 31, 1, 1, 1, 6);

    /// <summary>IF-MIB::ifHCOutOctets (64-bit).</summary>
    public static readonly Oid IfHCOutOctets = new(1, 3, 6, 1, 2, 1, 31, 1, 1, 1, 10);

    /// <summary>SNMPv2-MIB::sysUpTime.0.</summary>
    public static readonly Oid SysUpTime = new(1, 3, 6, 1, 2, 1, 1, 3, 0);
}
