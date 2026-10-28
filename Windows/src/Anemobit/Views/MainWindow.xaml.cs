using System.Windows;
using Anemobit.ViewModels;

namespace Anemobit.Views;

/// <summary>Main window. Uses the compact layout (statistics below the gauges) when narrower than <see cref="CompactWidth"/>.</summary>
public partial class MainWindow : Window
{
    public const double CompactWidth = 700;

    public static readonly DependencyProperty IsCompactProperty = DependencyProperty.Register(
        nameof(IsCompact), typeof(bool), typeof(MainWindow), new PropertyMetadata(false));

    private readonly Action _openSettings;

    public MainWindow(TrafficViewModel traffic, Action openSettings)
    {
        InitializeComponent();
        AcrylicBackdrop.Attach(this);
        DataContext = traffic;
        _openSettings = openSettings;
        SizeChanged += (_, e) => IsCompact = e.NewSize.Width < CompactWidth;
    }

    public bool IsCompact
    {
        get => (bool)GetValue(IsCompactProperty);
        set => SetValue(IsCompactProperty, value);
    }

    private void OnOpenSettings(object sender, RoutedEventArgs e) => _openSettings();
}
