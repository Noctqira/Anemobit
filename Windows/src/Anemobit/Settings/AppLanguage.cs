namespace Anemobit.Settings;

/// <summary>
/// Display language chosen in Settings. <see cref="System"/> follows the Windows display language.
/// Resources are chosen at launch, so a change takes effect after Anemobit restarts.
/// </summary>
public enum AppLanguage
{
    System,
    English,
    TraditionalChinese,
    Japanese,
    Korean,
}
