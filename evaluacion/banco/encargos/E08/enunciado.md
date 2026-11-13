Entregalo con escribir_propuesta, nombre exacto ventas.sql

Base de datos SQLite 3. Tablas: ventas(id, cliente_id, importe, fecha), clientes(id, nombre, pais).
La fecha es texto con formato AAAA-MM-DD.

Escribe una consulta que devuelva, por pais, los 3 clientes con mas importe total en 2025,
con su posicion en el ranking y que porcentaje representan sobre el total de su pais en 2025.

- Columnas, en este orden: pais, cliente_id, nombre, total, posicion, porcentaje.
- porcentaje va de 0 a 100, redondeado a 2 decimales.
- Ordena por pais y despues por posicion.
- Usa funciones de ventana.

El fichero contiene solo esa consulta. Debajo, en un comentario, di que indice crearias y por que,
con su sentencia CREATE INDEX.
