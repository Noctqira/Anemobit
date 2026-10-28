using System.Windows.Input;

namespace Anemobit.Infrastructure;

/// <summary><see cref="ICommand"/> implemented with delegates. Call <see cref="NotifyCanExecuteChanged"/> when the can-execute state changes.</summary>
public sealed class RelayCommand(Action execute, Func<bool>? canExecute = null) : ICommand
{
    public event EventHandler? CanExecuteChanged;

    public bool CanExecute(object? parameter) => canExecute?.Invoke() ?? true;

    public void Execute(object? parameter)
    {
        if (CanExecute(parameter))
        {
            execute();
        }
    }

    public void NotifyCanExecuteChanged() => CanExecuteChanged?.Invoke(this, EventArgs.Empty);
}
