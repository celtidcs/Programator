# E07: cola FIFO con límite en Lua

Se usa solo cuando las pruebas ocultas no dieron nota (apagadas, o sin Lua en el equipo). Se
puntúa **leyendo el código, sin ejecutarlo**, con lo mismo que miran las pruebas: el orden FIFO con
límite, que se pueda reutilizar, que las instancias sean independientes, las metatablas, las
globales y el coste.

## Qué se mira, y cuánto vale

Suma los puntos de lo que se cumple. Cada punto se da entero o no se da.

- **1**: `Cola.nueva(capacidad)` devuelve una instancia con `setmetatable` y los métodos van en la
  metatabla (`Cola.__index = Cola`), no en closures; el fichero termina con `return Cola`.
- **1**: el orden es FIFO: con capacidad 2, `meter("a")`, `meter("b")` dan `true`, un tercer
  `meter` da `false`, y `sacar()` da `"a"`, `"b"` y después `nil`; `tamano()` dice cuántos hay.
- **1**: `meter` y `sacar` son O(1): índices de cabeza y de cola, sin `table.remove(t, 1)` ni nada
  que desplace los elementos.
- **0,5**: se reutiliza tras vaciarse: los índices no se quedan desfasados, y meter otra vez tras
  sacarlo todo funciona y respeta el límite.
- **0,5**: al sacar, el hueco queda a `nil`, para no retener los valores ya sacados.
- **0,5**: dos colas son independientes: el estado (elementos, cabeza, cola, capacidad) va en cada
  instancia, no en el módulo.
- **0,5**: no crea variables globales: todo es `local`, también `Cola` y las funciones auxiliares.

## Qué baja la nota

- **0**, como con las pruebas: no entregó código, falta `Cola.nueva` o el `return Cola`, o tiene
  un error que con seguridad impide cargarlo. Si solo dudas, no restes por eso y dilo en los
  motivos.
- **Máximo 2**: la cola no tiene límite, o `meter` no devuelve `false` cuando está llena.

Una nota de esta rúbrica no es comparable con la de las pruebas ocultas: el informe dice cuál es
cuál.
