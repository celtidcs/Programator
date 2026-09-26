## En qué acierta y en qué no, medido

Cada fila lleva su cifra y la fecha en que se tomó. **Una fila sin las dos no entra**: la guía
afirmó durante meses que diagnosticar era «lo mejor que hace», y cuando se midió resultó 0 de 12.

| Clase de encargo | Medida | Fecha |
|---|---|---|
| Auditar buscando **formas**: números sueltos, métodos largos, clases que hacen de todo | 16 hallazgos ciertos de 18 en los dos ficheros cargados (6 de 6 y 10 de 12) | 24/09/2026 |
| Auditar código ajeno, **tomando las cinco auditorías juntas** | 27 hallazgos ciertos de 41, con una varianza enorme: de 6 de 6 a 0 de 4 | 24/09/2026 |
| **Función acotada**, con los requisitos en lista y un valor de referencia calculado aparte | 14 de 14 requisitos, 5 de 5 valores | 24/09/2026 |
| Auditar buscando **validaciones ausentes** o errores de significado | 0 accionables de 6 | 24/09/2026 |
| **Diagnosticar la causa** de un fallo | 0 de 12 | 24/09/2026 |
| Auditar un fichero que **crees limpio**, para confirmarlo | 0 útiles de 4; rellena con hallazgos inventados | 24/09/2026 |

**La regla que resume la tabla:** pídele que **reconozca formas**, nunca que **entienda
comportamientos**. «¿Qué hay aquí que viole esta regla?» sí. «¿Por qué esto se comporta así?» no.

Las fechas de la tabla son la fecha de publicación de cada medida, no la fecha individual de cada encargo. Todos estos datos se cierran el 24 de septiembre de 2026.

Origen de las cifras: `evaluacion/desempeno-uso-real-natureland.md`, catorce encargos sobre un
proyecto real verificados uno a uno contra el código.
