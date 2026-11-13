# E04: validador de configuración en TypeScript

Se usa solo cuando las pruebas ocultas no dieron nota (apagadas, o sin Node en el equipo). Se
puntúa **leyendo el código, sin ejecutarlo**, con lo mismo que miran las pruebas: los tipos
exportados, que acumule todos los errores y cada regla de validación, incluidas las trampas de
JavaScript (`NaN`, `Infinity`, `null` y los arrays).

## Qué se mira, y cuánto vale

Suma los puntos de lo que se cumple. Cada punto se da entero o no se da.

- **1**: exporta exactamente `type Resultado<T, E>` como unión discriminada por `ok`, la
  `interface Config` con `depurar` opcional y `function parsearConfig(entrada: unknown):
  Resultado<Config, string[]>`.
- **1**: acumula **todos** los errores en una lista y devuelve `{ ok: false, error }` con ella; no
  se detiene en el primero ni lanza excepciones. Con una configuración válida devuelve
  `{ ok: true, valor }` con los tres campos.
- **0,5**: rechaza lo que no es un objeto: `null`, `undefined`, una cadena, un número **y un
  array** (`typeof [] === "object"`, así que mirar solo `typeof` no basta).
- **0,5**: `puerto` tiene que ser un número entre 1 y 65535, y `NaN` no vale (comparar con `>=` y
  `<=` deja fuera `NaN` sin más; `typeof` solo no).
- **0,5**: `reintentos` tiene que ser entero y `>= 0`: rechaza `-1`, `1.5` e `Infinity` (por ejemplo
  con `Number.isInteger`).
- **0,5**: `host` tiene que ser una cadena no vacía.
- **0,5**: `depurar`, si viene, tiene que ser booleano; si no viene, vale.
- **0,5**: no hay `any`, y cada `as` lleva un comentario que explica por qué es seguro; al final
  hay tres ejemplos de uso comentados, uno válido y dos inválidos.

## Qué baja la nota

- **0**, como con las pruebas: no entregó código, faltan los tres exportados o tiene un error que
  con seguridad impide compilar en modo estricto (por ejemplo, leer una propiedad de `unknown` sin
  estrecharlo). Si solo dudas de si compila, no restes por eso y dilo en los motivos.
- **Máximo 2**: usa una biblioteca de validación externa (zod, yup…) en vez de escribirla.

Una nota de esta rúbrica no es comparable con la de las pruebas ocultas: el informe dice cuál es
cuál.
