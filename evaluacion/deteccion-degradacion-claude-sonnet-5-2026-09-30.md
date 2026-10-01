# Detección de posible degradación de rendimiento en peticiones consecutivas — Claude Sonnet 5

> Modelo: **Claude Sonnet 5** (`claude-sonnet-5`).
> Fecha de observación y redacción: **2026-09-30**.
> Contexto: sesión 104 de NatureLand, dirigiendo a Programator (`devstral-small-2-24b-Q4_K_M`) en
> cuatro encargos consecutivos de auditoría de «formas» sobre ficheros `.cs` distintos, todos con
> el mismo formato de petición ya rodado en sesiones anteriores.
> Esto NO sustituye a `incidencias-claude-agente2-natureland-2026-09-30.md` (que ya recoge el
> encargo 025, INC-N10, como incidencia puntual): este documento existe para señalar el **patrón**
> que conecta esa incidencia con las tres anteriores, no para repetir el detalle de una sola.

## Resumen

En una misma sesión, cuatro encargos de auditoría casi idénticos en formato y exigencia (mismo
esqueleto de instrucciones, mismas cuatro categorías a buscar, mismos avisos de qué no es un
hallazgo) fueron respondidos con calidad decreciente conforme el tiempo entre petición y petición
se acortaba. La cuarta respuesta incumplió de raíz una regla de formato obligatoria y una
exclusión explícita, con un artefacto técnico (un bloque de la propia respuesta duplicado dentro
de sí mismo) que no encaja bien con «el modelo se despistó» y sí podría apuntar a un problema de
generación o de concatenación en el arnés.

**Aviso de honestidad estadística, antes de nada**: son **4 muestras**, una por fichero, sin
repetición del mismo fichero con distinto espaciado para aislar la variable. No se puede
distinguir con esta única sesión si la causa es el espaciado entre peticiones, la complejidad del
fichero en sí, o simple varianza de un modelo local de 24B sin controlar `seed`/`temperature` desde
este lado (NatureLand no fija esos parámetros al encargar, solo escribe en el canal). Lo que sigue
es una observación con dos hipótesis razonables, no una causa demostrada.

## Los datos

| Encargo | Hora de entrega | Minutos desde la entrega anterior | Fichero auditado | Líneas del fichero | Literales numéricos sueltos en el fichero | Líneas de línea real citadas | Resultado verificado |
|---|---|---:|---|---:|---:|---|---|
| 022 | 21:34 | — (primero de la tanda) | `CameraRig.cs` | 316 | 45 | Sí, con desplazamiento pero presentes | 8 de 9 hallazgos ciertos — mejor precisión de toda la serie histórica |
| 023 | 21:56 | +22 | `CicloDiaNoche.cs` | 317 | 35 | Sí, **2 con la línea exacta** | 9 de 12 útiles |
| 024 | 22:04 | +8 | `InspectorEstrategico.cs` | 368 | 15 | **No — citó nombres de método en vez de líneas** | 6 de 6 hallazgos ciertos en el fondo (formato ya degradado, contenido todavía correcto) |
| 025 | 22:13 | +9 | `PantallaMapaCampania.cs` | 383 | **69** | **No — ni líneas ni miembros, un solo bloque** | **Hallazgo principal inservible**: sin ubicación, con 7 colores señalados que el encargo prohibía explícitamente, y un tramo de ~40 números **repetido dos veces dentro de la misma respuesta** |

Detalle verificado de cada entrega (cómo se verificó y qué se aplicó) en
`../../NatureLand/.gestor/canal-historico/historico-encargos.md`, encargos 022-025.

## Dos hipótesis, no excluyentes

**Hipótesis A — espaciado entre peticiones.** El primer encargo de la tanda (022) llegó tras un
hueco largo (la sesión llevaba horas en otras tareas antes de retomar Programator). Los tres
siguientes se sucedieron con 22, 8 y 9 minutos de diferencia — una cadencia bastante más rápida,
sin pausa real entre el cierre de un encargo y la publicación del siguiente. La adherencia al
formato (citar línea real) se mantiene en el primer hueco largo y en el segundo hueco todavía
amplio (22 min), y se pierde justo en los dos huecos cortos (8 y 9 min) — con el segundo de esos
dos además perdiendo calidad de contenido, no solo de formato.

**Hipótesis B — densidad de literales del fichero.** `PantallaMapaCampania.cs` tiene **69**
literales numéricos sueltos, el doble que el siguiente fichero más denso de la tanda (45, en
`CameraRig.cs`, que sin embargo dio el mejor resultado de toda la serie). Es plausible que, al
pedir «lista todo lo que está incrustado sin nombre», un fichero con muchos más literales que
enumerar empuje al modelo a un modo de volcado bruto en vez de una selección cuidadosa uno por uno
— lo que explicaría el abandono del formato de línea, aunque no explica por sí sola el bloque
duplicado dentro de la respuesta.

**Lo que no cuadra con ninguna de las dos por sí sola**: el encargo 024, con el fichero **menos**
denso de los cuatro (15 literales) y el hueco más corto hasta ese momento (8 min), ya perdió la
cita de línea real (aunque el contenido siguiera siendo correcto). Eso apunta a que el espaciado
pesa más que la densidad como variable aislada — pero con una muestra de 4 no es concluyente.

## El artefacto que más preocupa: contenido duplicado dentro de la misma respuesta

En el encargo 025, la lista de números del hallazgo `HARDCODEO` no es solo larga: un tramo de
aproximadamente 40 valores aparece **repetido dos veces seguidas, idéntico**, dentro de la misma
línea de respuesta. Esto no es el tipo de error que se explica con «el modelo se equivocó de
línea» (el patrón habitual, ya documentado, de desplazamientos de −5 a +9 sin aritmética simple).
Es más compatible con algo a nivel de generación o de ensamblado de la respuesta — por ejemplo, si
el arnés concatena dos pasadas parciales sobre el mismo fichero sin deduplicar, o si el propio
modelo, cerca de agotar su presupuesto de salida o de contexto disponible, repite un fragmento ya
generado en vez de continuar con contenido nuevo.

No tengo acceso a los logs internos de Programator para confirmar cuál de las dos es. Lo dejo
anotado porque, si es reproducible, es más fácil de depurar desde dentro (registrando cuántas
llamadas a herramienta o cuánta generación llevaba consumida el modelo en el momento de escribir
esa línea) que desde fuera.

## Qué pediría, si el equipo de Programator quiere investigarlo

1. **Registrar el espaciado real entre el cierre de un encargo y la llegada del siguiente**, no
   solo la hora de cada latido — para poder correlacionar de verdad cadencia y calidad en vez de
   estimarla a mano como aquí.
2. **Si hay contención de GPU/VRAM entre peticiones muy seguidas** (la propia `rendimiento.md` de
   esta carpeta ya documenta que el encaje de capas es sensible a cuánta caché reserva el motor):
   comprobar si una petición que llega mientras el motor todavía está liberando recursos de la
   anterior degrada la calidad de la siguiente generación, no solo su velocidad.
3. **Reproducir con el mismo fichero (`PantallaMapaCampania.cs`) en dos condiciones**: inmediatamente
   tras otro encargo (como pasó aquí) y con una pausa de varios minutos, mismo `seed`/`temperature`
   si el arnés los expone — para separar la hipótesis A de la B de una vez.
4. Este documento es una foto de una sola sesión. Si vuelve a pasar en otra, lo más útil es que
   quien lo detecte cree un documento nuevo con su propio nombre de modelo y fecha (mismo criterio
   que ya usa `incidencias-claude-agente2-natureland-2026-09-30.md`) en vez de acumular sesiones
   distintas en el mismo fichero — así cada observación queda fechada y atribuida a condiciones
   verificables de esa sesión concreta, sin mezclar circunstancias.

## Réplica del punto 3, desde Programator (2026-10-01)

Se reprodujo exactamente el experimento pedido: el mismo encargo de auditoría sobre
`PantallaMapaCampania.cs`, con `programator.exe` real, semilla fija (`42`) y temperatura por
defecto (`0.2`), en dos condiciones — inmediatamente después de otro encargo sobre el mismo motor
ya cargado, y en un motor recién arrancado sin ninguna petición previa.

**Ni la condición caliente ni la fría reprodujeron el colapso grave** (bloque único sin ubicación,
contenido duplicado dentro de sí mismo, colores excluidos incluidos) que describe este documento.
Una sola muestra por condición no permite descartar que vuelva a pasar — sigue haciendo falta más
repetición para eso —, pero si el espaciado por sí solo bastara para dispararlo, sería razonable
esperar verlo al menos en la condición caliente, y no apareció.

**Lo que sí se repitió en ambas condiciones, igual de mal, fue la imprecisión de línea de
INC-N08**: en los dos casos, los dos hallazgos de tipo HARDCODEO citaban una línea que no era la
real. En la condición caliente, sin un patrón de desplazamiento consistente (`380f`: dijo línea
200, está en la 106 — un error de +94; `0.0` del contador de repintado: dijo línea 220, está en la
297 — un error de −77, en sentido contrario al anterior). En la fría, `380f` otra vez mal citado
(dijo línea 187, está en la 106 — error de +81), y el segundo hallazgo (`0.6f`) citó además el
miembro equivocado, sobre un valor que aparece dos veces en el fichero sin que ninguna de las dos
coincidiera con lo dicho. Que el mismo defecto aparezca igual de mal con la VRAM
recién liberada que con el servidor caliente es la evidencia más directa disponible sin un banco
mucho mayor de que esa imprecisión concreta **no depende del estado del servidor**: apunta a una
limitación del modelo, no a contención de GPU ni a espaciado entre peticiones. Detalle completo y
la decisión que se tomó a partir de esto en `incidencias-claude-agente2-natureland-2026-09-30.md`,
notas bajo INC-N07, INC-N08 e INC-N10.
