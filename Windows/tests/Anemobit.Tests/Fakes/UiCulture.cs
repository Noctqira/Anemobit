using System.Globalization;

namespace Anemobit.Tests.Fakes;

/// <summary>Temporarily switches the UI and formatting cultures (restored when the test ends), so text assertions don't depend on the computer's language.</summary>
internal sealed class UiCulture : IDisposable
{
    private readonly CultureInfo _previousUi = CultureInfo.CurrentUICulture;
    private readonly CultureInfo _previous = CultureInfo.CurrentCulture;

    public UiCulture(string name)
    {
        var culture = name.Length == 0 ? CultureInfo.InvariantCulture : new CultureInfo(name);
        CultureInfo.CurrentUICulture = culture;
        CultureInfo.CurrentCulture = culture;
    }

    /// <summary>Development language (English, the default resx resources).</summary>
    public static UiCulture English() => new(string.Empty);

    public void Dispose()
    {
        CultureInfo.CurrentUICulture = _previousUi;
        CultureInfo.CurrentCulture = _previous;
    }
}
