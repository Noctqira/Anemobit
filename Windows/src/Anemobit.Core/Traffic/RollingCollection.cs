using System.Collections;

namespace Anemobit.Core.Traffic;

/// <summary>Fixed-capacity ring buffer; drops the oldest element when full. Enumerates oldest to newest.</summary>
public sealed class RollingCollection<T> : IReadOnlyCollection<T>
{
    private readonly T[] _storage;
    private int _head; // Position of the oldest element
    private int _count;

    public RollingCollection(int capacity)
    {
        ArgumentOutOfRangeException.ThrowIfNegativeOrZero(capacity);
        _storage = new T[capacity];
    }

    public int Capacity => _storage.Length;

    public int Count => _count;

    /// <summary>The newest element; <c>default</c> when empty.</summary>
    public T? Last => _count == 0 ? default : _storage[(_head + _count - 1) % _storage.Length];

    public void Add(T item)
    {
        if (_count < _storage.Length)
        {
            _storage[(_head + _count) % _storage.Length] = item;
            _count++;
        }
        else
        {
            _storage[_head] = item;
            _head = (_head + 1) % _storage.Length;
        }
    }

    public void Clear()
    {
        Array.Clear(_storage);
        _head = 0;
        _count = 0;
    }

    public IEnumerator<T> GetEnumerator()
    {
        for (var i = 0; i < _count; i++)
        {
            yield return _storage[(_head + i) % _storage.Length];
        }
    }

    IEnumerator IEnumerable.GetEnumerator() => GetEnumerator();
}
