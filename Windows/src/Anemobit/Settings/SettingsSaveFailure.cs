namespace Anemobit.Settings;

public enum SettingsSaveFailure
{
    /// <summary>settings.json can't be written.</summary>
    File,

    /// <summary>The community or passwords can't be written to Credential Manager.</summary>
    Credentials,
}
