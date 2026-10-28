using System.Text.Json.Serialization;

namespace Anemobit.Settings;

/// <summary>
/// Contents of settings.json (non-sensitive settings only). Split into device / snmp / display / floatingWindow;
/// security mode is stored as RouterOS's none / authorized / private.
/// </summary>
internal sealed class SettingsDocument
{
    public DeviceSection Device { get; set; } = new();

    public SnmpSection Snmp { get; set; } = new();

    public DisplaySection Display { get; set; } = new();

    public FloatingWindowSection FloatingWindow { get; set; } = new();

    internal sealed class DeviceSection
    {
        public string Host { get; set; } = string.Empty;

        public int Port { get; set; } = AppSettings.DefaultPort;

        public List<string> InterfaceNames { get; set; } = [];
    }

    internal sealed class SnmpSection
    {
        /// <summary>none / authorized / private.</summary>
        public string SecurityMode { get; set; } = "none";

        /// <summary>md5 / sha1 / sha256 / sha384 / sha512.</summary>
        public string AuthProtocol { get; set; } = "sha1";

        /// <summary>des / aes.</summary>
        public string PrivProtocol { get; set; } = "aes";
    }

    internal sealed class DisplaySection
    {
        public int RefreshSeconds { get; set; } = 1;

        /// <summary>bits / bytes.</summary>
        public string UnitStyle { get; set; } = "bits";

        public bool ChartsWhenMultiple { get; set; }

        public string? DownloadColor { get; set; }

        public string? UploadColor { get; set; }

        /// <summary>en / zh-Hant / ja / ko; null follows the system.</summary>
        public string? Language { get; set; }
    }

    internal sealed class FloatingWindowSection
    {
        public bool Visible { get; set; } = true;

        public bool AlwaysOnTop { get; set; } = true;

        public double Opacity { get; set; } = 0.7;

        public string Interface { get; set; } = string.Empty;

        public string? Monitor { get; set; }

        public int? X { get; set; }

        public int? Y { get; set; }

        public bool TrayHintShown { get; set; }
    }
}

[JsonSourceGenerationOptions(WriteIndented = true, PropertyNamingPolicy = JsonKnownNamingPolicy.CamelCase)]
[JsonSerializable(typeof(SettingsDocument))]
internal sealed partial class SettingsJsonContext : JsonSerializerContext;
