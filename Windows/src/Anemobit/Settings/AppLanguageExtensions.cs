using System.Globalization;

namespace Anemobit.Settings;

public static class AppLanguageExtensions
{
    /// <summary>Culture name of the language's resources, also stored in settings.json; null for <see cref="AppLanguage.System"/>.</summary>
    public static string? CultureName(this AppLanguage language) => language switch
    {
        AppLanguage.English => "en",
        AppLanguage.TraditionalChinese => "zh-Hant",
        AppLanguage.Japanese => "ja",
        AppLanguage.Korean => "ko",
        _ => null,
    };

    /// <summary>The language's name in that language (not translated); null for <see cref="AppLanguage.System"/>, whose title is translated.</summary>
    public static string? NativeName(this AppLanguage language) => language switch
    {
        AppLanguage.English => "English",
        AppLanguage.TraditionalChinese => "繁體中文",
        AppLanguage.Japanese => "日本語",
        AppLanguage.Korean => "한국어",
        _ => null,
    };

    /// <summary>UI culture to use; null follows the system.</summary>
    public static CultureInfo? ToCulture(this AppLanguage language) =>
        language.CultureName() is { } name ? CultureInfo.GetCultureInfo(name) : null;

    /// <summary>Reads a stored culture name (case-insensitive); anything else follows the system.</summary>
    public static AppLanguage FromCultureName(string? name) =>
        Enum.GetValues<AppLanguage>().FirstOrDefault(l => string.Equals(l.CultureName(), name, StringComparison.OrdinalIgnoreCase));
}
