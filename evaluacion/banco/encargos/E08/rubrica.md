# E08: los tres mejores clientes por país en SQLite

Se usa solo cuando las pruebas ocultas no dieron nota (apagadas, o sin SQLite en el equipo). Se
puntúa **leyendo la consulta, sin ejecutarla**, con lo mismo que miran las pruebas: que SQLite la
acepte, las columnas y su orden, el filtro de 2025, el ranking, el porcentaje sobre el total del
país y los países con menos de tres clientes.

## Qué se mira, y cuánto vale

Suma los puntos de lo que se cumple. Cada punto se da entero o no se da.

- **1**: el fichero tiene **una sola** consulta que SQLite 3 acepta (las pruebas ejecutan el
  fichero entero: otra sentencia que devuelva filas o un `CREATE INDEX` sin comentar lo rompen) y
  devuelve las columnas `pais, cliente_id, nombre, total, posicion, porcentaje`, en ese orden.
- **0,5**: solo cuenta las ventas de 2025: `fecha` entre `'2025-01-01'` y `'2025-12-31'`,
  `LIKE '2025-%'` o `strftime('%Y', fecha) = '2025'`, aplicado antes de sumar.
- **1**: el ranking es por país con una función de ventana (`ROW_NUMBER()`, `RANK()` o
  `DENSE_RANK() OVER (PARTITION BY pais ORDER BY total DESC)`) y se filtra `posicion <= 3` en una
  consulta exterior o en un CTE (en el `WHERE` de la misma consulta no se puede).
- **1**: el porcentaje es sobre el total **de todo el país** en 2025, calculado antes de quedarse
  con los tres primeros (por ejemplo `SUM(total) OVER (PARTITION BY pais)`), y no sobre la suma de
  los tres.
- **0,5**: el porcentaje no cae en la división entera (`importe` es `INTEGER`): multiplica por
  `100.0` o convierte a real, y lo redondea con `ROUND(..., 2)`.
- **0,5**: un país con menos de tres clientes sale con los que tenga (no hay un `JOIN` ni un
  filtro que lo deje fuera).
- **0,5**: ordena por `pais` y después por `posicion`, y debajo, en un comentario, propone un
  índice útil con su `CREATE INDEX` y su porqué (por ejemplo sobre `ventas(fecha, cliente_id)` o
  `ventas(cliente_id, fecha)`).

## Qué baja la nota

- **0**, como con las pruebas: no entregó la consulta, no es SQL de SQLite (`TOP`, `QUALIFY`,
  funciones de otro motor) o tiene un error que con seguridad SQLite rechaza. Si solo dudas, no
  restes por eso y dilo en los motivos.
- **Máximo 2**: no usa funciones de ventana.

Una nota de esta rúbrica no es comparable con la de las pruebas ocultas: el informe dice cuál es
cuál.
