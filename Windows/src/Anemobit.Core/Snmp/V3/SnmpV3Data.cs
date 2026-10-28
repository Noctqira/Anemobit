namespace Anemobit.Core.Snmp.V3;

/// <summary>Data part of an SNMPv3 message: a plaintext ScopedPDU, or encrypted bytes.</summary>
internal abstract record SnmpV3Data
{
    private SnmpV3Data()
    {
    }

    public sealed record Plaintext(ScopedPdu ScopedPdu) : SnmpV3Data;

    public sealed record Encrypted(byte[] Ciphertext) : SnmpV3Data
    {
        public bool Equals(Encrypted? other) => other is not null && Ciphertext.AsSpan().SequenceEqual(other.Ciphertext);

        public override int GetHashCode() => Ciphertext.Length;
    }
}
