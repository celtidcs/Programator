# E05: decorador de reintentos con espera exponencial en Python

Se usa solo cuando las pruebas ocultas no dieron nota (apagadas, o sin Python en el equipo). Se
puntúa **leyendo el código, sin ejecutarlo**, con lo mismo que miran las pruebas: la firma, la
serie de esperas, qué excepción se relanza y que sus propias pruebas no duerman de verdad.

## Qué se mira, y cuánto vale

Suma los puntos de lo que se cumple. Cada punto se da entero o no se da.

- **0,5**: la firma es `def reintentar(intentos=3, espera_inicial=0.1, excepciones=(Exception,),
  jitter=True)` y se usa como `@reintentar(...)`, con paréntesis.
- **0,5**: conserva el nombre y el docstring de la función decorada (`functools.wraps`).
- **1**: devuelve el valor de la función cuando acaba saliendo bien, también tras fallar, y
  funciona con funciones que no devuelven nada.
- **1**: si se agotan los intentos, relanza **la misma** excepción del último intento (`raise` sin
  argumentos dentro del `except`, o guardarla y `raise` de ella), no una nueva ni envuelta.
- **0,5**: solo reintenta las excepciones de `excepciones`; las demás salen a la primera, sin
  esperar.
- **0,5**: sin jitter, las esperas son `espera_inicial · 2^k`: 0,1, 0,2, 0,4…, y no hay espera
  después del último intento. Con `jitter=True`, cada espera tiene una parte aleatoria (`random`).
- **0,5**: `intentos=0` (o negativo) no acaba en `raise None` ni en un `TypeError` sin sentido:
  se rechaza al decorar con un `ValueError`, o la función no se llama, o cualquier otra cosa
  razonable.
- **0,5**: trae pruebas con `unittest` en el mismo fichero, y no duermen de verdad: interceptan
  `time.sleep` (con `mock.patch`) o usan esperas de cero.

## Qué baja la nota

- **0**, como con las pruebas: no entregó código, el decorador no se llama `reintentar` o tiene un
  error que con seguridad impide importar el módulo. Si solo dudas, no restes por eso y dilo en
  los motivos.
- **Máximo 2**: usa una biblioteca externa de reintentos (tenacity, backoff…).
- **Restar 1** si sus pruebas, tal como están, dormirían segundos reales.

Una nota de esta rúbrica no es comparable con la de las pruebas ocultas: el informe dice cuál es
cuál.
