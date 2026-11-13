# U04: un inventario en Lua que pierde productos

Corresponde a la fila de desempeño «Diagnosticar la causa de un fallo», donde Devstral acertó 0 de
12 en el uso real en que se midió por primera vez (24/09/2026).

## La causa

`quitar` pone `self.productos[i] = nil`, lo que deja un hueco en medio de la secuencia. Dos cosas
dejan de funcionar a la vez:

1. `ipairs` recorre desde 1 y **se detiene en el primer `nil`**. Tras quitar el segundo producto,
   el informe solo ve el primero, aunque el tercero siga en la tabla.
2. El operador `#` sobre una tabla con huecos devuelve **cualquier borde** de la secuencia, no
   necesariamente el último. Por eso `anadir` puede escribir en la posición del hueco (y el
   producto aparece) o detrás del último (y queda oculto tras el hueco), según cómo estén
   repartidas la parte de array y la parte de hash de la tabla.

## El arreglo

`table.remove(self.productos, i)`, que desplaza los elementos siguientes y mantiene la secuencia
sin huecos. Alternativas válidas: guardar los productos en una tabla indexada por nombre y recorrerla
con `pairs` (perdiendo el orden), o reconstruir la lista sin el elemento.

## Qué vale cada nota

- **5**: explica los dos mecanismos (ipairs se detiene en el hueco y `#` es indefinido con huecos)
  y da un arreglo correcto.
- **4**: explica bien `ipairs` y el arreglo, y atribuye lo de las altas posteriores a `#` sin
  precisar por qué.
- **3**: identifica que el problema es poner `nil` en medio de la lista y da el arreglo, sin
  explicar ninguno de los dos mecanismos.
- **2**: da el arreglo correcto con una causa equivocada o inventada.
- **1**: señala otra causa (por ejemplo, la metatabla, `__index` o el `return true` dentro del
  bucle) y un arreglo que no lo resuelve.
- **0**: no responde, o afirma que el código es correcto.
