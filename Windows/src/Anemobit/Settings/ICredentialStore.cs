namespace Anemobit.Settings;

/// <summary>Secure storage for sensitive data (community, SNMPv3 passwords): Windows Credential Manager in production, an in-memory version in tests.</summary>
public interface ICredentialStore
{
    string? Read(string name);

    /// <summary>Writes; returns true on success.</summary>
    bool Write(string name, string secret);

    /// <summary>Deletes; a missing entry also counts as success.</summary>
    bool Delete(string name);
}
