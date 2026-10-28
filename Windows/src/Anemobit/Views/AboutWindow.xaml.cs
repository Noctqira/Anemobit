using System.Diagnostics;
using System.Globalization;
using System.Reflection;
using System.Windows;
using Anemobit.Resources;

namespace Anemobit.Views;

/// <summary>About Anemobit. The version comes from the assembly info (Version in Anemobit.csproj).</summary>
public partial class AboutWindow : Window
{
    public AboutWindow()
    {
        InitializeComponent();
        AcrylicBackdrop.Attach(this);
        var version = Assembly.GetExecutingAssembly().GetName().Version;
        VersionText.Text = string.Format(CultureInfo.CurrentCulture, Strings.VersionFormat, version?.ToString(3) ?? "?");
    }

    private void OnOpenLink(object sender, RoutedEventArgs e)
    {
        if (sender is FrameworkElement { Tag: string url } && Uri.TryCreate(url, UriKind.Absolute, out var uri) && uri.Scheme == Uri.UriSchemeHttps)
        {
            Process.Start(new ProcessStartInfo(uri.AbsoluteUri) { UseShellExecute = true })?.Dispose();
        }
    }

    private void OnClose(object sender, RoutedEventArgs e) => Close();
}
