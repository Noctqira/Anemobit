using System.ComponentModel;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Media;
using Anemobit.Settings;
using Anemobit.ViewModels;

namespace Anemobit.Views;

/// <summary>
/// Settings window. PasswordBox contents can't be bound, so they're synced with the view model here; colors are picked from swatches (WPF has no built-in color picker).
/// </summary>
public partial class SettingsWindow : Window
{
    /// <summary>Available colors: the default download / upload colors (light and dark) plus common colors.</summary>
    private static readonly string[] Swatches =
    [
        "#4FD1E8", "#0E8FB0", "#3B82F6", "#8B5CF6", "#E879F9", "#B03CC4",
        "#EF4444", "#F97316", "#EAB308", "#8FD94F", "#22C55E", "#94A3B8",
    ];

    private readonly SettingsViewModel _model;

    public SettingsWindow(SettingsViewModel model)
    {
        InitializeComponent();
        AcrylicBackdrop.Attach(this);
        _model = model;
        DataContext = model;

        CommunityBox.Password = model.Community;
        AuthPasswordBox.Password = model.AuthPassword;
        PrivPasswordBox.Password = model.PrivPassword;
        // The v2c community and the v3 user name are the same field: sync changes from the text box to the PasswordBox.
        model.PropertyChanged += OnModelChanged;
        BuildSwatches(DownloadSwatches, isDownload: true);
        BuildSwatches(UploadSwatches, isDownload: false);
        UpdateActivity();
        Closed += (_, _) =>
        {
            model.PropertyChanged -= OnModelChanged;
            model.Dispose();
        };
    }

    private void OnModelChanged(object? sender, PropertyChangedEventArgs e)
    {
        switch (e.PropertyName)
        {
            case nameof(SettingsViewModel.Community) when CommunityBox.Password != _model.Community:
                CommunityBox.Password = _model.Community;
                break;
            case nameof(SettingsViewModel.ActivityKind):
                UpdateActivity();
                break;
            case nameof(SettingsViewModel.DownloadColor):
            case nameof(SettingsViewModel.UploadColor):
                BuildSwatches(DownloadSwatches, isDownload: true);
                BuildSwatches(UploadSwatches, isDownload: false);
                break;
            default:
                break;
        }
    }

    private void OnCommunityChanged(object sender, RoutedEventArgs e) => _model.Community = CommunityBox.Password;

    private void OnAuthPasswordChanged(object sender, RoutedEventArgs e) => _model.AuthPassword = AuthPasswordBox.Password;

    private void OnPrivPasswordChanged(object sender, RoutedEventArgs e) => _model.PrivPassword = PrivPasswordBox.Password;

    private void UpdateActivity()
    {
        (ActivityIcon.Text, ActivityIcon.Foreground, ActivityText.Foreground) = _model.ActivityKind switch
        {
            ActivityKind.Success => ("", Brushes.SeaGreen, (Brush)Brushes.SeaGreen),
            ActivityKind.Failure => ("", Brushes.IndianRed, Brushes.IndianRed),
            _ => (string.Empty, (Brush)FindResource("SecondaryTextBrush"), (Brush)FindResource("SecondaryTextBrush")),
        };
    }

    /// <summary>A row of swatches; the selected color gets an outline. When not customized, the theme default swatch isn't marked.</summary>
    private void BuildSwatches(ItemsControl host, bool isDownload)
    {
        var current = isDownload ? _model.DownloadColor : _model.UploadColor;
        var panel = new WrapPanel();
        foreach (var hex in Swatches)
        {
            var selected = string.Equals(hex, current, StringComparison.OrdinalIgnoreCase);
            var swatch = new Button
            {
                Width = 26,
                Height = 26,
                Margin = new Thickness(0, 0, 8, 6),
                Cursor = System.Windows.Input.Cursors.Hand,
                ToolTip = hex,
                Template = SwatchTemplate(HexColor.ToColor(hex)!.Value, selected),
            };
            System.Windows.Automation.AutomationProperties.SetName(swatch, hex);
            swatch.Click += (_, _) =>
            {
                if (isDownload)
                {
                    _model.DownloadColor = hex;
                }
                else
                {
                    _model.UploadColor = hex;
                }
            };
            panel.Children.Add(swatch);
        }
        host.Items.Clear();
        host.Items.Add(panel);
    }

    private ControlTemplate SwatchTemplate(Color color, bool selected)
    {
        var ring = new FrameworkElementFactory(typeof(Border));
        ring.SetValue(Border.CornerRadiusProperty, new CornerRadius(13));
        ring.SetValue(Border.BorderThicknessProperty, new Thickness(selected ? 2 : 0));
        ring.SetValue(Border.BorderBrushProperty, (Brush)FindResource("AccentBrush"));
        ring.SetValue(Border.PaddingProperty, new Thickness(selected ? 3 : 0));
        var dot = new FrameworkElementFactory(typeof(Border));
        dot.SetValue(Border.CornerRadiusProperty, new CornerRadius(13));
        dot.SetValue(Border.BackgroundProperty, new SolidColorBrush(color));
        ring.AppendChild(dot);
        return new ControlTemplate(typeof(Button)) { VisualTree = ring };
    }
}
