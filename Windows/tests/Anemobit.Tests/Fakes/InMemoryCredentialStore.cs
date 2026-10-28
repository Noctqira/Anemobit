using Anemobit.Settings;

namespace Anemobit.Tests.Fakes;

/// <summary>In-memory credential store; <see cref="FailWrites"/> simulates write failures.</summary>
internal sealed class InMemoryCredentialStore : ICredentialStore
{
    public Dictionary<string, string> Secrets { get; } = new(StringComparer.Ordinal);

    public bool FailWrites { get; set; }

    public string? Read(string name) => Secrets.GetValueOrDefault(name);

    public bool Write(string name, string secret)
    {
        if (FailWrites)
        {
            return false;
        }
        Secrets[name] = secret;
        return true;
    }

    public bool Delete(string name)
    {
        Secrets.Remove(name);
        return true;
    }
}
