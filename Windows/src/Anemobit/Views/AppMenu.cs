using System.Windows.Controls;
using Anemobit.Resources;

namespace Anemobit.Views;

/// <summary>Right-click menu for the notification area icon and floating window: Open Main Window, Settings, About, Hide / Show Floating Window, Quit.</summary>
internal static class AppMenu
{
    public static ContextMenu Create(AppCommands commands, bool floatingVisible)
    {
        var menu = new ContextMenu();
        menu.Items.Add(Item(Strings.OpenMainWindow, commands.OpenMainWindow));
        menu.Items.Add(Item(Strings.SettingsEllipsis, commands.OpenSettings));
        menu.Items.Add(Item(Strings.About, commands.OpenAbout));
        menu.Items.Add(new Separator());
        menu.Items.Add(floatingVisible
            ? Item(Strings.HideFloatingWindow, () => commands.SetFloatingWindowVisible(false))
            : Item(Strings.ShowFloatingWindowMenu, () => commands.SetFloatingWindowVisible(true)));
        menu.Items.Add(new Separator());
        menu.Items.Add(Item(Strings.Quit, commands.Quit));
        return menu;
    }

    private static MenuItem Item(string header, Action action)
    {
        var item = new MenuItem { Header = header };
        item.Click += (_, _) => action();
        return item;
    }
}
