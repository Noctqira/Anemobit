using System.Globalization;
using Anemobit.Text;

namespace Anemobit.Tests.Text;

public class UptimeTextTests
{
    [Theory]
    [InlineData(0, "0 minutes", "0分鐘", "0分", "0분")]
    [InlineData(59, "0 minutes", "0分鐘", "0分", "0분")]
    [InlineData(60, "1 minute", "1分鐘", "1分", "1분")]
    [InlineData(3_660, "1 hour, 1 minute", "1小時1分鐘", "1時間1分", "1시간 1분")]
    [InlineData(86_399, "23 hours, 59 minutes", "23小時59分鐘", "23時間59分", "23시간 59분")]
    [InlineData((86_400 * 12) + (3_600 * 5) + 120, "12 days, 5 hours", "12天5小時", "12日5時間", "12일 5시간")]
    [InlineData(86_400, "1 day", "1天", "1日", "1일")]
    public void Format_Duration_UsesUpToTwoLocalizedUnits(int seconds, string english, string chinese, string japanese, string korean)
    {
        var duration = TimeSpan.FromSeconds(seconds);
        Assert.Equal(english, UptimeText.Format(duration, CultureInfo.InvariantCulture));
        Assert.Equal(chinese, UptimeText.Format(duration, new CultureInfo("zh-Hant")));
        Assert.Equal(chinese, UptimeText.Format(duration, new CultureInfo("zh-TW"))); // Common system region codes must find the translation too
        Assert.Equal(japanese, UptimeText.Format(duration, new CultureInfo("ja-JP")));
        Assert.Equal(korean, UptimeText.Format(duration, new CultureInfo("ko-KR")));
    }

    [Fact]
    public void Format_UnsupportedLanguage_FallsBackToEnglish()
    {
        Assert.Equal("1 hour, 1 minute", UptimeText.Format(TimeSpan.FromSeconds(3_660), new CultureInfo("de-DE")));
    }
}
