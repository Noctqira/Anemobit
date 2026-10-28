using System.Diagnostics.CodeAnalysis;
using System.Net;
using System.Net.Sockets;

namespace Anemobit.Core.Formatting;

/// <summary>Device address: IPv4, IPv6 or host name. Used to validate input in Settings and to get the host string for connecting.</summary>
/// <param name="Kind">Address kind.</param>
/// <param name="Host">Host used for connecting (without brackets or a trailing dot).</param>
public sealed record DeviceAddress(DeviceAddressKind Kind, string Host)
{
    private const int MaximumHostNameLength = 253;
    private const int MaximumLabelLength = 63;

    /// <summary>Trims surrounding whitespace; IPv6 may be written as <c>[2001:db8::1]</c>, and a trailing <c>.</c> on a host name is removed.</summary>
    public static bool TryParse([NotNullWhen(true)] string? text, [NotNullWhen(true)] out DeviceAddress? address)
    {
        address = null;
        var value = text?.Trim() ?? string.Empty;
        if (value.Length == 0)
        {
            return false;
        }

        if (value.StartsWith('[') && value.EndsWith(']'))
        {
            value = value[1..^1];
            if (!value.Contains(':', StringComparison.Ordinal) || !IsIPv6(value))
            {
                return false;
            }
            address = new DeviceAddress(DeviceAddressKind.IPv6, value);
            return true;
        }
        if (value.Contains(':', StringComparison.Ordinal))
        {
            if (!IsIPv6(value))
            {
                return false;
            }
            address = new DeviceAddress(DeviceAddressKind.IPv6, value);
            return true;
        }
        if (IsDottedQuad(value))
        {
            address = new DeviceAddress(DeviceAddressKind.IPv4, value);
            return true;
        }

        if (value.EndsWith('.'))
        {
            value = value[..^1];
        }
        if (!IsHostName(value))
        {
            return false;
        }
        address = new DeviceAddress(DeviceAddressKind.HostName, value);
        return true;
    }

    private static bool IsIPv6(string value) =>
        IPAddress.TryParse(value, out var ip) && ip.AddressFamily == AddressFamily.InterNetworkV6;

    /// <summary>Four decimal numbers 0–255.</summary>
    private static bool IsDottedQuad(string value)
    {
        var parts = value.Split('.');
        return parts.Length == 4 && parts.All(part =>
            part.Length is >= 1 and <= 3 && part.All(char.IsAsciiDigit) && int.Parse(part, System.Globalization.CultureInfo.InvariantCulture) <= 255);
    }

    /// <summary>
    /// RFC 1123: total length ≤ 253; each label is 1–63 letters, digits or <c>-</c>, not starting or ending with <c>-</c>;
    /// the last label can't be all digits (so a mistyped IP isn't taken as a host name).
    /// </summary>
    private static bool IsHostName(string value)
    {
        if (value.Length > MaximumHostNameLength)
        {
            return false;
        }
        var labels = value.Split('.');
        var validLabels = labels.All(label =>
            label.Length is >= 1 and <= MaximumLabelLength
            && label.All(c => char.IsAsciiLetterOrDigit(c) || c == '-')
            && label[0] != '-'
            && label[^1] != '-');
        return validLabels && !labels[^1].All(char.IsAsciiDigit);
    }
}
