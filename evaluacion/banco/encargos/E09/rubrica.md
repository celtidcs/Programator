# E09: un generador que corrompe lo ya entregado

## La causa

El generador entrega siempre **la misma lista**. `yield lote` no copia nada: quien recibe el lote
tiene una referencia al mismo objeto, y `lote.clear()` lo vacía justo después. La prueba del autor
pasa porque mide `len(l)` en el momento de recibir cada lote, antes del `clear()`. `list(...)`
guarda las referencias y, al terminar, todas apuntan a la misma lista vacía. Con seis elementos y
lotes de tres no queda resto, y por eso se ven dos listas vacías.

## El arreglo

Entregar una lista nueva cada vez: `lote = []` en lugar de `lote.clear()`, o `yield list(lote)`
(o `lote[:]`) antes de vaciar. Cualquiera de las dos es correcta; la primera evita una copia.

## Qué vale cada nota

- **5**: identifica que se entrega la misma lista (aliasing o referencia compartida), explica por
  qué la prueba del autor no lo detecta y da un arreglo correcto.
- **4**: causa y arreglo correctos, sin explicar por qué la prueba del autor pasa.
- **3**: arreglo correcto con una explicación vaga o parcialmente errónea de la causa.
- **2**: señala la línea de `clear()` sin entender por qué es un problema, o propone un arreglo que
  no lo resuelve del todo.
- **1**: diagnostica otra causa (por ejemplo, el último lote o el tipo de `items`).
- **0**: no responde, o afirma que el código es correcto.
