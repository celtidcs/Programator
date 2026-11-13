Encargo de codigo. Entregalo con escribir_propuesta, nombre exacto LectorLotes.cs

En C# moderno, escribe una clase que lea un fichero de texto grande por lotes de N lineas:

- public sealed class LectorLotes : IAsyncDisposable, en el espacio de nombres global.
- Constructor: public LectorLotes(string ruta)
- Metodo: IAsyncEnumerable<IReadOnlyList<string>> LeerLotesAsync(int tamanoLote, CancellationToken ct = default)
- Debe respetar la cancelacion, liberar el StreamReader y no cargar el fichero entero en memoria.
- El ultimo lote puede ser mas pequeno.
- Valida los argumentos y lanza la excepcion adecuada del framework.

Publica un resumen de lo entregado.
