using System.Windows.Media;
using Anemobit.Settings;

namespace Anemobit.Tests.Settings;

public class HexColorTests
{
    [Theory]
    [InlineData("#0e8fb0", "#0E8FB0")]
    [InlineData("4FD1E8", "#4FD1E8")]
    [InlineData(" #abcdef ", "#ABCDEF")]
    [InlineData("#12345", null)]
    [InlineData("#GGGGGG", null)]
    [InlineData("red", null)]
    [InlineData(null, null)]
    public void Normalize_Text_ReturnsUppercaseHashForm(string? text, string? expected)
    {
        Assert.Equal(expected, HexColor.Normalize(text));
    }

    [Fact]
    public void ToColorAndBack_RoundTrips()
    {
        Assert.Equal(Color.FromRgb(0x0E, 0x8F, 0xB0), HexColor.ToColor("#0E8FB0"));
        Assert.Equal("#0E8FB0", HexColor.FromColor(Color.FromRgb(0x0E, 0x8F, 0xB0)));
    }
}
