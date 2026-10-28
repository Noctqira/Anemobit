using System.Globalization;
using Anemobit.Core.Snmp;
using Anemobit.Resources;
using Anemobit.Settings;

namespace Anemobit.Text;

/// <summary>Settings input problems → UI text.</summary>
public static class ProblemText
{
    public static string Describe(SecurityProblem problem) => problem switch
    {
        SecurityProblem.MissingCommunity => Strings.EnterCommunity,
        SecurityProblem.MissingUserName => Strings.EnterUserName,
        SecurityProblem.AuthPasswordTooShort => string.Format(CultureInfo.CurrentCulture, Strings.AuthPasswordTooShortFormat, UsmCredentials.MinimumPasswordLength),
        _ => string.Format(CultureInfo.CurrentCulture, Strings.PrivPasswordTooShortFormat, UsmCredentials.MinimumPasswordLength),
    };
}
