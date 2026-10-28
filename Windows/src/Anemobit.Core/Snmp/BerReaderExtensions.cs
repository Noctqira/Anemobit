using Anemobit.Core.Ber;

namespace Anemobit.Core.Snmp;

/// <summary><see cref="BerReader"/> extension methods for reading SNMP message fields.</summary>
internal static class BerReaderExtensions
{
    public static long ReadInteger(this BerReader reader) =>
        SnmpValue.Decode(reader) is SnmpValue.Integer32 integer
            ? integer.Value
            : throw new BerException(BerError.InvalidContent, BerTag.Integer);

    public static byte[] ReadOctetString(this BerReader reader) =>
        SnmpValue.Decode(reader) is SnmpValue.OctetString octets
            ? octets.Value.ToArray()
            : throw new BerException(BerError.InvalidContent, BerTag.OctetString);
}
