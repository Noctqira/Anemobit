namespace Anemobit.Core.Snmp;

/// <summary>PDU type; the values are BER tags.</summary>
internal enum PduType : byte
{
    Get = 0xA0,
    GetNext = 0xA1,
    Response = 0xA2,
    Set = 0xA3,
    GetBulk = 0xA5,
    Report = 0xA8,
}
