using Anemobit.Core.Snmp;

namespace Anemobit.Core.Tests.Fakes;

internal static class SnmpMessageTestExtensions
{
    /// <summary>Produces an encoded response to this request.</summary>
    public static byte[] Response(
        this SnmpMessage request,
        IReadOnlyList<VarBind> varBinds,
        int? requestId = null,
        long errorStatus = 0,
        long errorIndex = 0) =>
        new SnmpMessage(
            request.Community,
            new Pdu(PduType.Response, requestId ?? request.Pdu.RequestId, errorStatus, errorIndex, varBinds))
        .Encode();
}
