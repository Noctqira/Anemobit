using Anemobit.Settings;

namespace Anemobit.Tests.Settings;

public class AppLanguageExtensionsTests
{
    [Theory]
    [InlineData(AppLanguage.System, null)]
    [InlineData(AppLanguage.English, "en")]
    [InlineData(AppLanguage.TraditionalChinese, "zh-Hant")]
    [InlineData(AppLanguage.Japanese, "ja")]
    [InlineData(AppLanguage.Korean, "ko")]
    public void CultureName_EachLanguage_MatchesResourceCulture(AppLanguage language, string? expected) =>
        Assert.Equal(expected, language.CultureName());

    [Theory]
    [InlineData(AppLanguage.English, "English")]
    [InlineData(AppLanguage.TraditionalChinese, "繁體中文")]
    [InlineData(AppLanguage.Japanese, "日本語")]
    [InlineData(AppLanguage.Korean, "한국어")]
    public void NativeName_EachLanguage_IsWrittenInThatLanguage(AppLanguage language, string expected) =>
        Assert.Equal(expected, language.NativeName());

    [Fact]
    public void NativeName_System_IsNull() =>
        Assert.Null(AppLanguage.System.NativeName());

    [Fact]
    public void FromCultureName_EveryLanguage_RoundTrips()
    {
        foreach (var language in Enum.GetValues<AppLanguage>())
        {
            Assert.Equal(language, AppLanguageExtensions.FromCultureName(language.CultureName()));
        }
    }

    [Theory]
    [InlineData("ZH-hant", AppLanguage.TraditionalChinese)]
    [InlineData("EN", AppLanguage.English)]
    [InlineData("fr", AppLanguage.System)]
    [InlineData("zh-Hans", AppLanguage.System)]
    [InlineData("", AppLanguage.System)]
    public void FromCultureName_OtherSpellings_IgnoreCaseAndFallBackToSystem(string name, AppLanguage expected) =>
        Assert.Equal(expected, AppLanguageExtensions.FromCultureName(name));

    [Fact]
    public void ToCulture_System_IsNull() =>
        Assert.Null(AppLanguage.System.ToCulture());

    [Fact]
    public void ToCulture_Japanese_IsJapaneseCulture() =>
        Assert.Equal("ja", AppLanguage.Japanese.ToCulture()?.Name);
}
