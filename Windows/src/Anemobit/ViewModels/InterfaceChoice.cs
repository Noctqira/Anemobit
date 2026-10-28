using Anemobit.Infrastructure;

namespace Anemobit.ViewModels;

/// <summary>One (selectable) row in the Settings interface list.</summary>
public sealed class InterfaceChoice(string name, bool isSelected, Action<InterfaceChoice> onToggled) : ObservableObject
{
    private bool _isSelected = isSelected;

    public string Name { get; } = name;

    public bool IsSelected
    {
        get => _isSelected;
        set
        {
            if (SetProperty(ref _isSelected, value))
            {
                onToggled(this);
            }
        }
    }

    /// <summary>Used when the view model syncs state; doesn't trigger <c>onToggled</c>.</summary>
    internal void SetSelectedSilently(bool value)
    {
        _isSelected = value;
        OnPropertyChanged(nameof(IsSelected));
    }
}
