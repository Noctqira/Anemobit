using System.Globalization;
using Anemobit.Core.Formatting;
using Anemobit.Core.Traffic;
using Anemobit.Infrastructure;
using Anemobit.Resources;

namespace Anemobit.ViewModels;

/// <summary>Live rate and recent history for one interface (shared by cards, the floating window, and the notification area flyout).</summary>
public sealed class InterfaceTraffic : ObservableObject
{
    /// <summary>Charts and statistics cover the last 5 minutes.</summary>
    public static readonly TimeSpan HistoryWindow = TimeSpan.FromMinutes(5);

    /// <summary>Buffer capacity is sized for the fastest 1-second interval.</summary>
    public const int HistoryCapacity = 300;

    private const string Placeholder = "--";

    private readonly TimeProvider _time;
    private RateUnitStyle _unitStyle;
    private TrafficSample? _latest;
    private TrafficStatistics? _statistics;
    private int _revision;

    public InterfaceTraffic(string name, RateUnitStyle unitStyle, TimeProvider? timeProvider = null)
    {
        Name = name;
        _unitStyle = unitStyle;
        _time = timeProvider ?? TimeProvider.System;
    }

    public string Name { get; }

    public TrafficSample? Latest => _latest;

    /// <summary>Oldest to newest; at 5- or 10-second intervals this covers more than 5 minutes, so the chart filters by time.</summary>
    public RollingCollection<TrafficSample> History { get; } = new(HistoryCapacity);

    /// <summary>Increments with each sample; the chart redraws from it.</summary>
    public int Revision => _revision;

    public RateUnitStyle UnitStyle
    {
        get => _unitStyle;
        set
        {
            if (SetProperty(ref _unitStyle, value))
            {
                NotifyDisplayChanged();
            }
        }
    }

    /// <summary>Average and peak over the last 5 minutes.</summary>
    public TrafficStatistics? Statistics => _statistics;

    public string CurrentIn => Format(_latest?.InBps);

    public string CurrentOut => Format(_latest?.OutBps);

    public string AverageIn => Format(_statistics?.AverageIn);

    public string AverageOut => Format(_statistics?.AverageOut);

    public string PeakIn => Format(_statistics?.PeakIn);

    public string PeakOut => Format(_statistics?.PeakOut);

    /// <summary>Gauge maximum: this direction's peak over the last 5 minutes, rounded up in the display unit (e.g. 2 MB/s, 50 Mbps).</summary>
    public double GaugeMaxIn => RateScale.Axis(_statistics?.PeakIn ?? 0, _unitStyle).Maximum;

    public double GaugeMaxOut => RateScale.Axis(_statistics?.PeakOut ?? 0, _unitStyle).Maximum;

    public string GaugeMaxInText => MaxText(GaugeMaxIn);

    /// <summary>Gauge fill fraction (0–1).</summary>
    public double InFraction => Fraction(_latest?.InBps, GaugeMaxIn);

    public double OutFraction => Fraction(_latest?.OutBps, GaugeMaxOut);

    public string GaugeMaxOutText => MaxText(GaugeMaxOut);

    /// <summary>Value and unit in the center of the gauge ("--" and empty when there's no data).</summary>
    public string InValue => Split(_latest?.InBps).Value;

    public string InUnit => Split(_latest?.InBps).Unit;

    public string OutValue => Split(_latest?.OutBps).Value;

    public string OutUnit => Split(_latest?.OutBps).Unit;

    /// <summary>"↓ 12.3 Mbps ↑ 1.2 Mbps"; "↓ … ↑ …" when there's no data.</summary>
    public string Summary => _latest is { } sample ? RateFormatter.Summary(sample, _unitStyle) : "↓ … ↑ …";

    /// <summary>Samples from the last 5 minutes.</summary>
    public IReadOnlyList<TrafficSample> RecentSamples()
    {
        var cutoff = _time.GetUtcNow() - HistoryWindow;
        return [.. History.Where(s => s.Timestamp >= cutoff)];
    }

    public void Append(TrafficSample sample)
    {
        ArgumentNullException.ThrowIfNull(sample);
        _latest = sample;
        History.Add(sample);
        _statistics = TrafficStatistics.From(RecentSamples());
        _revision++;
        OnPropertiesChanged(nameof(Latest), nameof(Statistics), nameof(Revision));
        NotifyDisplayChanged();
    }

    /// <summary>Clears the current rate when the connection fails (history is kept).</summary>
    public void ClearLatest()
    {
        if (_latest is null)
        {
            return;
        }
        _latest = null;
        OnPropertyChanged(nameof(Latest));
        NotifyDisplayChanged();
    }

    private void NotifyDisplayChanged() => OnPropertiesChanged(
        nameof(CurrentIn), nameof(CurrentOut), nameof(AverageIn), nameof(AverageOut), nameof(PeakIn), nameof(PeakOut),
        nameof(GaugeMaxIn), nameof(GaugeMaxOut), nameof(GaugeMaxInText), nameof(GaugeMaxOutText), nameof(InFraction), nameof(OutFraction),
        nameof(InValue), nameof(InUnit), nameof(OutValue), nameof(OutUnit), nameof(Summary));

    private string Format(double? bps) => bps is { } value ? RateFormatter.Format(value, _unitStyle) : Placeholder;

    private (string Value, string Unit) Split(double? bps) =>
        bps is { } value ? RateFormatter.Split(value, _unitStyle) : (Placeholder, " ");

    private static double Fraction(double? bps, double maximum) =>
        bps is { } value && maximum > 0 ? Math.Clamp(value / maximum, 0, 1) : 0;

    private string MaxText(double maximum) =>
        string.Format(CultureInfo.CurrentCulture, Strings.MaxFormat, RateFormatter.Format(maximum, _unitStyle));
}
