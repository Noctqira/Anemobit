namespace Anemobit.Settings;

/// <summary>
/// Security mode, matching RouterOS <c>/snmp community</c> security. The settings file stores none / authorized / private,
/// and the UI shows None / Auth / Priv (not translated).
/// </summary>
public enum SecurityMode
{
    /// <summary>SNMPv2c, community only.</summary>
    None,

    /// <summary>SNMPv3 authNoPriv: authentication only.</summary>
    Authorized,

    /// <summary>SNMPv3 authPriv: authentication + privacy.</summary>
    Private,
}
