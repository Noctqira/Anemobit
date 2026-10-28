namespace Anemobit.ViewModels;

/// <summary>Dashboard connection state.</summary>
public abstract record ConnectionState
{
    private ConnectionState()
    {
    }

    public sealed record NotConfigured : ConnectionState;

    public sealed record Connecting : ConnectionState;

    public sealed record Connected : ConnectionState;

    public sealed record Failed(Exception Error, TimeSpan RetryIn) : ConnectionState;
}
