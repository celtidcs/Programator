using System.Runtime.CompilerServices;

public sealed class LectorLotes : IAsyncDisposable
{
    private readonly StreamReader _lector;

    public LectorLotes(string ruta)
    {
        ArgumentException.ThrowIfNullOrEmpty(ruta);
        _lector = new StreamReader(ruta);
    }

    public IAsyncEnumerable<IReadOnlyList<string>> LeerLotesAsync(int tamanoLote, CancellationToken ct = default)
    {
        ArgumentOutOfRangeException.ThrowIfNegativeOrZero(tamanoLote);
        return Leer(tamanoLote, ct);
    }

    private async IAsyncEnumerable<IReadOnlyList<string>> Leer(int tamanoLote, [EnumeratorCancellation] CancellationToken ct)
    {
        var lote = new List<string>(tamanoLote);
        string? linea;
        // ReadLineAsync no mira el token si la línea ya está en el búfer: se comprueba en cada vuelta.
        while ((linea = await _lector.ReadLineAsync(ct)) is not null)
        {
            ct.ThrowIfCancellationRequested();
            lote.Add(linea);
            if (lote.Count == tamanoLote)
            {
                yield return lote;
                lote = new List<string>(tamanoLote);
            }
        }
        if (lote.Count > 0)
        {
            yield return lote;
        }
    }

    public ValueTask DisposeAsync()
    {
        _lector.Dispose();
        return ValueTask.CompletedTask;
    }
}
