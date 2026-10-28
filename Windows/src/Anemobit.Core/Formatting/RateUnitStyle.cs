namespace Anemobit.Core.Formatting;

/// <summary>How rates are shown.</summary>
public enum RateUnitStyle
{
    /// <summary>Bits, decimal: bps, Kbps, Mbps, Gbps (same as RouterOS).</summary>
    Bits,

    /// <summary>Bytes, binary (1024): B/s, KB/s, MB/s, GB/s.</summary>
    Bytes,
}
