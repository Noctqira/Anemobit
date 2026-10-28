namespace Anemobit.Settings;

/// <summary>Settings couldn't be saved; see <see cref="Failure"/> for why.</summary>
public sealed class SettingsSaveException : Exception
{
    public SettingsSaveException(SettingsSaveFailure failure, Exception? innerException = null)
        : base($"Could not save settings ({failure})", innerException)
    {
        Failure = failure;
    }

    public SettingsSaveFailure Failure { get; }
}
