using System.IO;

namespace Anemobit.Tests.Fakes;

/// <summary>Temporary folder for tests, deleted at the end.</summary>
internal sealed class TemporaryDirectory : IDisposable
{
    public TemporaryDirectory()
    {
        Path = System.IO.Path.Combine(System.IO.Path.GetTempPath(), "Anemobit.Tests", Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(Path);
    }

    public string Path { get; }

    public string SettingsFile => System.IO.Path.Combine(Path, "settings.json");

    public void Dispose()
    {
        try
        {
            Directory.Delete(Path, recursive: true);
        }
        catch (IOException)
        {
            // Leftover temporary files don't affect test results.
        }
    }
}
