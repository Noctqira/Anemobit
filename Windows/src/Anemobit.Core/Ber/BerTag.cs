namespace Anemobit.Core.Ber;

/// <summary>BER tag constants.</summary>
internal static class BerTag
{
    public const byte Integer = 0x02;
    public const byte OctetString = 0x04;
    public const byte Null = 0x05;
    public const byte Oid = 0x06;
    public const byte Sequence = 0x30;

    // SMIv2 application types
    public const byte IpAddress = 0x40;
    public const byte Counter32 = 0x41;
    public const byte Gauge32 = 0x42;
    public const byte TimeTicks = 0x43;
    public const byte Counter64 = 0x46;

    // SNMPv2 varbind exceptions (context-specific, primitive)
    public const byte NoSuchObject = 0x80;
    public const byte NoSuchInstance = 0x81;
    public const byte EndOfMibView = 0x82;
}
