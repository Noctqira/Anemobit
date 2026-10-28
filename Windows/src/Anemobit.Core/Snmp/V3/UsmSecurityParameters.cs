namespace Anemobit.Core.Snmp.V3;

/// <summary>UsmSecurityParameters (RFC 3414 §2.4). <see cref="ToString"/> excludes the user name and authentication / privacy parameters.</summary>
internal sealed record UsmSecurityParameters(
    byte[]? EngineId = null,
    uint EngineBoots = 0,
    uint EngineTime = 0,
    byte[]? UserName = null,
    byte[]? AuthParams = null,
    byte[]? PrivParams = null)
{
    public byte[] EngineId { get; init; } = EngineId ?? [];

    public byte[] UserName { get; init; } = UserName ?? [];

    public byte[] AuthParams { get; init; } = AuthParams ?? [];

    public byte[] PrivParams { get; init; } = PrivParams ?? [];

    public bool Equals(UsmSecurityParameters? other) =>
        other is not null
        && EngineId.AsSpan().SequenceEqual(other.EngineId)
        && EngineBoots == other.EngineBoots
        && EngineTime == other.EngineTime
        && UserName.AsSpan().SequenceEqual(other.UserName)
        && AuthParams.AsSpan().SequenceEqual(other.AuthParams)
        && PrivParams.AsSpan().SequenceEqual(other.PrivParams);

    public override int GetHashCode()
    {
        var hash = new HashCode();
        hash.AddBytes(EngineId);
        hash.Add(EngineBoots);
        hash.Add(EngineTime);
        return hash.ToHashCode();
    }

    public override string ToString() =>
        $"UsmSecurityParameters {{ EngineId = {Convert.ToHexString(EngineId)}, EngineBoots = {EngineBoots}, EngineTime = {EngineTime} }}";
}
