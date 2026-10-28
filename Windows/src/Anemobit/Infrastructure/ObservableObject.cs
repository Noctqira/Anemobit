using System.ComponentModel;
using System.Runtime.CompilerServices;

namespace Anemobit.Infrastructure;

/// <summary>Base class for view models: implements <see cref="INotifyPropertyChanged"/>.</summary>
public abstract class ObservableObject : INotifyPropertyChanged
{
    public event PropertyChangedEventHandler? PropertyChanged;

    /// <summary>Writes and notifies only when the value changes; returns whether it changed.</summary>
    protected bool SetProperty<T>(ref T field, T value, [CallerMemberName] string? propertyName = null)
    {
        if (EqualityComparer<T>.Default.Equals(field, value))
        {
            return false;
        }
        field = value;
        OnPropertyChanged(propertyName);
        return true;
    }

    protected void OnPropertyChanged([CallerMemberName] string? propertyName = null) =>
        PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(propertyName));

    /// <summary>Notifies several derived properties at once.</summary>
    protected void OnPropertiesChanged(params ReadOnlySpan<string> propertyNames)
    {
        foreach (var name in propertyNames)
        {
            OnPropertyChanged(name);
        }
    }
}
