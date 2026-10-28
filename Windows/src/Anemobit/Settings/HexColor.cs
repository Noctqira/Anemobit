using System.Globalization;
using System.Windows.Media;

namespace Anemobit.Settings;

/// <summary>Color string "#RRGGBB" in the settings file.</summary>
public static class HexColor
{
    /// <summary>Accepts "#RRGGBB" or "RRGGBB" (any case) and returns uppercase "#RRGGBB"; null when the format is wrong.</summary>
    public static string? Normalize(string? text)
    {
        var value = text?.Trim().TrimStart('#');
        return value is { Length: 6 } && value.All(char.IsAsciiHexDigit)
            ? "#" + value.ToUpperInvariant()
            : null;
    }

    public static Color? ToColor(string? text)
    {
        if (Normalize(text) is not { } hex)
        {
            return null;
        }
        var rgb = int.Parse(hex.AsSpan(1), NumberStyles.HexNumber, CultureInfo.InvariantCulture);
        return Color.FromRgb((byte)(rgb >> 16), (byte)(rgb >> 8), (byte)rgb);
    }

    public static string FromColor(Color color) => $"#{color.R:X2}{color.G:X2}{color.B:X2}";
}
