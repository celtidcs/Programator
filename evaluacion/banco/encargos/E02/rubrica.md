# E02: diseño de un servicio de sincronización sin conexión

Mide si sabe razonar un diseño: componentes, conflictos, casos de largo plazo y riesgos, con los
supuestos declarados.

## Lo que debe aparecer

1. **Componentes** con una responsabilidad cada uno: almacenamiento local en el cliente, cola o
   registro de cambios pendientes, servicio de sincronización, almacenamiento en el servidor y
   autenticación.
2. **Edición concurrente.** Lo que separa un buen diseño: para texto libre editado sin conexión,
   una estructura que fusione sin perder ediciones (un CRDT de texto) o, como mínimo, una fusión
   por operaciones con historia. «Gana el último que escribe» pierde trabajo del usuario: solo es
   aceptable si lo reconoce como pérdida y lo justifica. Transformación operacional (OT) sin
   servidor central siempre disponible es una mala elección para clientes que pasan días sin
   conexión; si la elige, debe decir cómo lo resuelve.
3. **Tres semanas sin conexión.** Qué hace el servidor con la historia que el cliente no ha visto
   (compactación, instantáneas, límite de retención) y qué pasa si la historia ya se compactó.
4. **El riesgo** nombrado es real y el motivo es concreto.
5. **Supuestos declarados**, y menos de 400 palabras sin código.

## Qué vale cada nota

- **5**: los cinco puntos, con una solución de conflictos que no pierde ediciones y una respuesta
  concreta al caso de tres semanas.
- **4**: los cinco puntos, con una solución de conflictos razonable pero con un hueco que no
  reconoce.
- **3**: estructura correcta, pero la solución de conflictos pierde ediciones sin decirlo, o el
  caso de tres semanas queda en generalidades.
- **2**: falta alguno de los puntos 2 o 3, o el diseño tiene una contradicción interna.
- **1**: una lista de tecnologías sin razonamiento.
- **0**: no responde, o escribe código pese a la prohibición.
