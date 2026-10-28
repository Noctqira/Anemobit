namespace Anemobit.Core.Snmp;

/// <summary>GETBULK walks and interface lookup.</summary>
public sealed partial class SnmpClient
{
    /// <summary>Walks the <paramref name="root"/> subtree with GETBULK and returns all varbinds (in OID order).</summary>
    /// <exception cref="SnmpException">OIDs returned by the agent don't increase (<see cref="SnmpError.UnexpectedResponse"/>), or the request failed.</exception>
    public async Task<IReadOnlyList<VarBind>> WalkAsync(
        Oid root, int maxRepetitions = 20, CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(root);
        var results = new List<VarBind>();
        var cursor = root;
        while (true)
        {
            var binds = await GetBulkAsync([cursor], maxRepetitions, cancellationToken: cancellationToken).ConfigureAwait(false);
            if (binds.Count == 0)
            {
                return results;
            }

            foreach (var bind in binds)
            {
                if (bind.Value is SnmpValue.EndOfMibView || !root.IsPrefixOf(bind.Oid) || bind.Oid == root)
                {
                    return results;
                }
                // OIDs returned by the agent must increase, or this would loop forever.
                if (bind.Oid <= cursor)
                {
                    throw SnmpException.UnexpectedResponse($"OID not increasing: {bind.Oid}");
                }
                results.Add(bind);
                cursor = bind.Oid;
            }
        }
    }

    /// <summary>Lists all interfaces (walks ifName; falls back to ifDescr if the agent has no ifName).</summary>
    public async Task<IReadOnlyList<SnmpInterface>> ListInterfacesAsync(CancellationToken cancellationToken = default)
    {
        var interfaces = await InterfacesAsync(SnmpOids.IfName, cancellationToken).ConfigureAwait(false);
        if (interfaces.Count == 0)
        {
            interfaces = await InterfacesAsync(SnmpOids.IfDescr, cancellationToken).ConfigureAwait(false);
        }
        return interfaces;
    }

    /// <summary>Resolves an interface name (e.g. <c>pppoe-out1</c>) to its ifIndex.</summary>
    /// <exception cref="SnmpException">The name wasn't found (<see cref="SnmpError.InterfaceNotFound"/>), or the request failed.</exception>
    public async Task<uint> ResolveInterfaceIndexAsync(string name, CancellationToken cancellationToken = default)
    {
        var indexes = await ResolveInterfaceIndexesAsync([name], cancellationToken).ConfigureAwait(false);
        return indexes[0];
    }

    /// <summary>Resolves several interface names in one walk; indexes are returned in the same order as <paramref name="names"/>.</summary>
    /// <exception cref="SnmpException">Any name wasn't found (<see cref="SnmpError.InterfaceNotFound"/>), or the request failed.</exception>
    public async Task<IReadOnlyList<uint>> ResolveInterfaceIndexesAsync(
        IReadOnlyList<string> names, CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(names);
        var interfaces = await ListInterfacesAsync(cancellationToken).ConfigureAwait(false);
        return
        [
            .. names.Select(name =>
                interfaces.FirstOrDefault(i => i.Name == name)?.Index ?? throw SnmpException.InterfaceNotFound(name)),
        ];
    }

    private async Task<List<SnmpInterface>> InterfacesAsync(Oid column, CancellationToken cancellationToken)
    {
        var binds = await WalkAsync(column, cancellationToken: cancellationToken).ConfigureAwait(false);
        var interfaces = new List<SnmpInterface>();
        foreach (var bind in binds)
        {
            if (bind.Oid.SuffixAfter(column) is [var index] && bind.Value.StringValue is { } name)
            {
                interfaces.Add(new SnmpInterface(index, name));
            }
        }
        return interfaces;
    }
}
