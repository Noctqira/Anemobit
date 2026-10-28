using System.Text;

namespace Anemobit.Core.Formatting;

/// <summary>
/// Filters the device address and port the user types: whatever keyboard, input method or paste is used, only allowed characters remain.
/// Full-width letters, digits and periods, and the CJK full stop are converted to ASCII first. <see cref="DeviceAddress"/> decides whether the format is valid.
/// </summary>
public static class InputFilter
{
    /// <summary>Host names are at most 253 characters; IPv6 with brackets and an interface name also fits.</summary>
    public const int HostMaximumLength = 253;

    /// <summary>Ports have at most 5 digits (65535).</summary>
    public const int PortMaximumLength = 5;

    /// <summary>Characters used by IPv4, IPv6 (including <c>[...]</c> and <c>%interface</c>) and host names: letters, digits, <c>.-:[]%</c>.</summary>
    public static string Host(string text)
    {
        var filtered = Normalize(text).Where(c => char.IsAsciiLetterOrDigit(c) || ".-:[]%".Contains(c, StringComparison.Ordinal));
        return new string([.. filtered.Take(HostMaximumLength)]);
    }

    /// <summary>Keeps digits only.</summary>
    public static string Port(string text) =>
        new([.. Normalize(text).Where(char.IsAsciiDigit).Take(PortMaximumLength)]);

    private static string Normalize(string text)
    {
        ArgumentNullException.ThrowIfNull(text);
        // NFKC converts full-width letters, digits and symbols to half-width; the CJK full stop "。" and the half-width "｡" are also treated as ".".
        return text.Normalize(NormalizationForm.FormKC).Replace('。', '.').Replace('｡', '.');
    }
}
