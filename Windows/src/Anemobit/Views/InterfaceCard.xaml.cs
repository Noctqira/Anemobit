using System.Windows;
using System.Windows.Controls;

namespace Anemobit.Views;

/// <summary>Card for one interface. When <see cref="Compact"/>, statistics move below the gauges.</summary>
public partial class InterfaceCard : UserControl
{
    public static readonly DependencyProperty CompactProperty = DependencyProperty.Register(
        nameof(Compact), typeof(bool), typeof(InterfaceCard), new PropertyMetadata(false, (d, _) => ((InterfaceCard)d).UpdateLayoutMode()));

    public static readonly DependencyProperty ShowsChartProperty = DependencyProperty.Register(
        nameof(ShowsChart), typeof(bool), typeof(InterfaceCard), new PropertyMetadata(true));

    public InterfaceCard()
    {
        InitializeComponent();
        UpdateLayoutMode();
    }

    public bool Compact
    {
        get => (bool)GetValue(CompactProperty);
        set => SetValue(CompactProperty, value);
    }

    public bool ShowsChart
    {
        get => (bool)GetValue(ShowsChartProperty);
        set => SetValue(ShowsChartProperty, value);
    }

    private void UpdateLayoutMode()
    {
        if (Compact)
        {
            StatsColumn.Width = new GridLength(0);
            Grid.SetRow(Stats, 1);
            Grid.SetColumn(Stats, 0);
            Grid.SetColumnSpan(Stats, 2);
            Stats.Width = double.NaN;
            Stats.Margin = new Thickness(0, 16, 0, 0);
            DownloadGauge.MaxHeight = UploadGauge.MaxHeight = 170;
        }
        else
        {
            StatsColumn.Width = GridLength.Auto;
            Grid.SetRow(Stats, 0);
            Grid.SetColumn(Stats, 2);
            Grid.SetColumnSpan(Stats, 1);
            Stats.Width = 300;
            Stats.Margin = new Thickness(24, 0, 0, 0);
            DownloadGauge.MaxHeight = UploadGauge.MaxHeight = 200;
        }
    }
}
