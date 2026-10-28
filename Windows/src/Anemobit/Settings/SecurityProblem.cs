namespace Anemobit.Settings;

/// <summary>Why the security settings can't connect (text localized by <see cref="Text.ProblemText"/>).</summary>
public enum SecurityProblem
{
    MissingCommunity,
    MissingUserName,
    AuthPasswordTooShort,
    PrivPasswordTooShort,
}
