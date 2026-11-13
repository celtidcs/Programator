# E06: lector de un fichero por lotes en C#

Se usa solo cuando las pruebas ocultas no dieron nota (apagadas, o sin .NET en el equipo). Se
puntúa **leyendo el código, sin ejecutarlo**, con lo mismo que miran las pruebas: la firma, el
tamaño de los lotes, que lo ya entregado no cambie, la validación, la cancelación y que no cargue
el fichero entero.

## Qué se mira, y cuánto vale

Suma los puntos de lo que se cumple. Cada punto se da entero o no se da.

- **1**: `public sealed class LectorLotes : IAsyncDisposable` en el espacio de nombres global, con
  `public LectorLotes(string ruta)` y `IAsyncEnumerable<IReadOnlyList<string>>
  LeerLotesAsync(int tamanoLote, CancellationToken ct = default)`, y `DisposeAsync` libera lo que
  abrió.
- **1**: lee línea a línea (`ReadLineAsync` sobre un `StreamReader`) y entrega lotes de
  `tamanoLote` líneas; el último puede ser más pequeño (7 líneas de 3 en 3 dan 3, 3 y 1), un
  fichero vacío no da ningún lote y un lote mayor que el fichero lo devuelve entero.
- **1**: cada lote es una lista **nueva**: no reutiliza la misma y la vacía con `Clear()` después
  de entregarla, porque eso cambia los lotes que ya tiene quien llama.
- **0,5**: un `tamanoLote` de cero o negativo lanza `ArgumentOutOfRangeException` (vale al llamar
  o al empezar a enumerar).
- **0,5**: respeta la cancelación: comprueba el token en el bucle
  (`ct.ThrowIfCancellationRequested()`, o se lo pasa a la lectura) y usa
  `[EnumeratorCancellation]` en el parámetro.
- **0,5**: no carga el fichero entero: nada de `File.ReadAllLines`, `ReadAllText` ni `ReadToEnd`.
- **0,5**: valida `ruta` en el constructor (nula o vacía lanza `ArgumentException` o
  `ArgumentNullException`) y el `StreamReader` se libera aunque la enumeración se corte (con
  `await using`, `using` o `finally`).

## Qué baja la nota

- **0**, como con las pruebas: no entregó código, la clase o el método no tienen la firma pedida o
  tiene un error que con seguridad impide compilar. Si solo dudas, no restes por eso y dilo en los
  motivos.
- **Máximo 2**: carga el fichero entero en memoria por otra vía y lo trocea después.

Una nota de esta rúbrica no es comparable con la de las pruebas ocultas: el informe dice cuál es
cuál.
