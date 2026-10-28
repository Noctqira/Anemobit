namespace Anemobit.Tests.Fakes;

/// <summary>Clock with a fixed (manually adjustable) wall-clock time.</summary>
internal sealed class FixedTimeProvider(DateTimeOffset now) : TimeProvider
{
    public DateTimeOffset Now { get; set; } = now;

    public override DateTimeOffset GetUtcNow() => Now;
}
