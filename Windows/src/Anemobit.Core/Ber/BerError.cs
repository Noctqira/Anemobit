namespace Anemobit.Core.Ber;

/// <summary>Why BER decoding failed.</summary>
internal enum BerError
{
    Truncated,
    IndefiniteLength,
    LengthTooLarge,
    UnexpectedTag,
    UnknownTag,
    InvalidContent,
    IntegerOverflow,
    InvalidOid,
}
