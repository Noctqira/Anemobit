using System.Text;
using Anemobit.Core.Snmp;

namespace Anemobit.Core.Tests.Fakes;

/// <summary>Simulates an SNMP agent's GET / GETNEXT / GETBULK behavior with a sorted MIB table.</summary>
internal sealed class FakeAgent
{
    private readonly Lock _lock = new();
    private readonly Dictionary<Oid, SnmpValue> _mib;

    public FakeAgent(IReadOnlyDictionary<Oid, SnmpValue>? mib = null)
    {
        _mib = mib is null ? [] : new Dictionary<Oid, SnmpValue>(mib);
    }

    public SnmpValue? this[Oid oid]
    {
        get
        {
            lock (_lock)
            {
                return _mib.GetValueOrDefault(oid);
            }
        }
    }

    /// <summary>RouterOS-style interface table: ether1, ether2, bridge, pppoe-out1 (index 7), wlan1 (index 12).</summary>
    public static Dictionary<Oid, SnmpValue> RouterOSInterfaceMib()
    {
        (uint Index, string Name)[] names = [(1, "ether1"), (2, "ether2"), (3, "bridge"), (7, "pppoe-out1"), (12, "wlan1")];
        var mib = new Dictionary<Oid, SnmpValue> { [SnmpOids.SysUpTime] = new SnmpValue.TimeTicks(100) };
        foreach (var (index, name) in names)
        {
            mib[SnmpOids.IfDescr.Appending(index)] = Octets(name);
            mib[SnmpOids.IfName.Appending(index)] = Octets(name);
            mib[SnmpOids.IfHCInOctets.Appending(index)] = new SnmpValue.Counter64(0);
            mib[SnmpOids.IfHCOutOctets.Appending(index)] = new SnmpValue.Counter64(0);
        }
        return mib;
    }

    public static SnmpValue.OctetString Octets(string text) => new([.. Encoding.UTF8.GetBytes(text)]);

    public void Update(IReadOnlyDictionary<Oid, SnmpValue> values)
    {
        lock (_lock)
        {
            foreach (var (oid, value) in values)
            {
                _mib[oid] = value;
            }
        }
    }

    public void Remove(Oid oid)
    {
        lock (_lock)
        {
            _mib.Remove(oid);
        }
    }

    public SnmpMessage Respond(SnmpMessage request)
    {
        Dictionary<Oid, SnmpValue> snapshot;
        lock (_lock)
        {
            snapshot = new(_mib);
        }
        var sorted = snapshot.Keys.Order().ToList();

        VarBind Next(Oid oid)
        {
            var key = sorted.FirstOrDefault(candidate => oid < candidate);
            return key is null ? new VarBind(oid, new SnmpValue.EndOfMibView()) : new VarBind(key, snapshot[key]);
        }

        var pdu = request.Pdu;
        var binds = new List<VarBind>();
        switch (pdu.Type)
        {
            case PduType.Get:
                binds.AddRange(pdu.VarBinds.Select(b =>
                    new VarBind(b.Oid, snapshot.GetValueOrDefault(b.Oid) ?? new SnmpValue.NoSuchInstance())));
                break;
            case PduType.GetNext:
                binds.AddRange(pdu.VarBinds.Select(b => Next(b.Oid)));
                break;
            case PduType.GetBulk:
                var nonRepeaters = (int)pdu.ErrorStatus;
                var maxRepetitions = (int)pdu.ErrorIndex;
                binds.AddRange(pdu.VarBinds.Take(nonRepeaters).Select(b => Next(b.Oid)));
                var cursors = pdu.VarBinds.Skip(nonRepeaters).Select(b => b.Oid).ToArray();
                for (var r = 0; r < maxRepetitions && cursors.Length > 0; r++)
                {
                    for (var i = 0; i < cursors.Length; i++)
                    {
                        var bind = Next(cursors[i]);
                        binds.Add(bind);
                        cursors[i] = bind.Oid;
                    }
                }
                break;
            default:
                break;
        }
        return new SnmpMessage(request.Community, new Pdu(PduType.Response, pdu.RequestId, 0, 0, binds));
    }
}
