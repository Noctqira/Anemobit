namespace Anemobit.Views;

/// <summary>Actions shared by the floating window, notification area icon and flyout (provided by <see cref="App"/>).</summary>
public sealed record AppCommands(
    Action OpenMainWindow,
    Action OpenSettings,
    Action OpenAbout,
    Action<bool> SetFloatingWindowVisible,
    Action Quit);
