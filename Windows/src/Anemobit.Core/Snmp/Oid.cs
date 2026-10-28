using System.Collections.Immutable;
using System.Diagnostics.CodeAnalysis;
using System.Globalization;

namespace Anemobit.Core.Snmp;

/// <summary>SNMP Object Identifier; each arc is a 32-bit unsigned integer.</summary>
public sealed class Oid : IEquatable<Oid>, IComparable<Oid>
{
    public Oid(params ReadOnlySpan<uint> arcs)
    {
        Arcs = [.. arcs];
    }

    public ImmutableArray<uint> Arcs { get; }

    public static bool operator ==(Oid? left, Oid? right) => left?.Equals(right) ?? right is null;

    public static bool operator !=(Oid? left, Oid? right) => !(left == right);

    public static bool operator <(Oid left, Oid right) => Compare(left, right) < 0;

    public static bool operator >(Oid left, Oid right) => Compare(left, right) > 0;

    public static bool operator <=(Oid left, Oid right) => Compare(left, right) <= 0;

    public static bool operator >=(Oid left, Oid right) => Compare(left, right) >= 0;

    /// <summary>Parses <c>1.3.6.1</c> or <c>.1.3.6.1</c> (ASCII digits only).</summary>
    public static bool TryParse([NotNullWhen(true)] string? text, [NotNullWhen(true)] out Oid? oid)
    {
        oid = null;
        var span = text.AsSpan();
        if (span.StartsWith('.'))
        {
            span = span[1..];
        }
        if (span.IsEmpty)
        {
            return false;
        }

        var arcs = new List<uint>();
        foreach (var range in span.Split('.'))
        {
            var part = span[range];
            if (part.IsEmpty || part.ContainsAnyExceptInRange('0', '9')
                || !uint.TryParse(part, NumberStyles.None, CultureInfo.InvariantCulture, out var arc))
            {
                return false;
            }
            arcs.Add(arc);
        }
        oid = new Oid([.. arcs]);
        return true;
    }

    public Oid Appending(uint arc) => new([.. Arcs, arc]);

    public bool IsPrefixOf(Oid other)
    {
        ArgumentNullException.ThrowIfNull(other);
        return other.Arcs.AsSpan().StartsWith(Arcs.AsSpan());
    }

    /// <summary>The arcs left after removing <paramref name="prefix"/>; null if <paramref name="prefix"/> isn't a prefix.</summary>
    public uint[]? SuffixAfter(Oid prefix)
    {
        ArgumentNullException.ThrowIfNull(prefix);
        return prefix.IsPrefixOf(this) ? Arcs.AsSpan()[prefix.Arcs.Length..].ToArray() : null;
    }

    /// <summary>Compares arcs lexicographically (used during walks to check that OIDs advance).</summary>
    public int CompareTo(Oid? other) => Compare(this, other);

    public bool Equals(Oid? other) => other is not null && Arcs.AsSpan().SequenceEqual(other.Arcs.AsSpan());

    public override bool Equals(object? obj) => obj is Oid other && Equals(other);

    public override int GetHashCode()
    {
        var hash = new HashCode();
        foreach (var arc in Arcs)
        {
            hash.Add(arc);
        }
        return hash.ToHashCode();
    }

    public override string ToString() => string.Join('.', Arcs);

    private static int Compare(Oid? left, Oid? right) => (left, right) switch
    {
        (null, null) => 0,
        (null, _) => -1,
        (_, null) => 1,
        _ => left.Arcs.AsSpan().SequenceCompareTo(right.Arcs.AsSpan()),
    };
}
