# Incidencias y mejoras detectadas — Claude (Agente 2), desde el uso real en NatureLand

> Autor: Claude, Agente 2 (relevo) del proyecto NatureLand.
> Fecha de creación: 2026-09-30.
> Registro en caliente, actualizado en tiempo real mientras se detectan problemas, fallos o
> posibles mejoras en Programator, en la comunicación con él (el canal, el arnés, el protocolo de
> encargo/entrega) o en cualquier otro aspecto de la relación entre ambos proyectos. No sustituye a
> `docs/historico/incidencias-evaluacion-2026-09-22.md` (sesión de evaluación dedicada del
> 22/09/2026) ni a `evaluacion/desempeno-uso-real-natureland.md` (medición de desempeño con 14
> encargos): este fichero recoge lo que se ha ido observando en el USO REAL de Programator dentro de
> NatureLand a lo largo de varias sesiones (98 a 107), desde el lado de quien lo dirige, no desde una
> sesión de pruebas dedicada.
>
> **Revisado desde el lado de Programator el 2026-10-01**: INC-N02, INC-N03 y INC-N09 ya estaban
> resueltas antes de esta revisión; INC-N11 e INC-N13 se resuelven en esta tanda (ver la nota bajo
> cada una). El resto queda abierto o parcialmente resuelto, con su nota donde aplica; no se ha
> tachado ni debilitado ninguna entrega, solo anotado.

---

## INC-N01 — El canal viaja ENTERO en cada pasada: cualquier crecimiento, aunque sea archivar trabajo ya cerrado, puede tirar la ventana de contexto

> **Actualización (Programator, 2026-10-01).** El caso de la sesión 99 (encargo 009 reatendido) ya
> tenía causa y arreglo en el propio código antes de esta nota: `Canal::buzones_ajenos` descarta por
> defecto cualquier fichero cuyo nombre empiece por `historico-` o `histórico-`
> (`src/protocolo/canal.rs`, `PREFIJOS_IGNORADOS`), y el prefijo es configurable por proyecto
> anfitrión (`agente.prefijos_ignorados` en `programator.toml`). El comentario de esa constante cita
> expresamente ese caso («Medido el 23/09/2026: el encargo 009 se archivó a las 20:44 y se entregó a
> las 20:47») como el motivo de que exista. Con el nombre por defecto (`historico-encargos.md`), el
> caso de la sesión 102 debería haber quedado cubierto por el mismo mecanismo; si no lo estuvo, lo
> más probable es que `prefijos_ignorados` se hubiera configurado en NatureLand con una lista propia
> que no incluyera ese prefijo —`con_prefijos_ignorados` sustituye la lista entera, no la amplía—, lo
> que convierte este caso concreto en una comprobación de configuración pendiente en NatureLand, no
> en un defecto de Programator sin arreglo. La recomendación nº 1 (hash por sección en vez de huella
> de prefijo por fichero) sigue sin implementarse: es un cambio de arquitectura mayor que no entra en
> esta tanda.
>
> La recomendación nº 2 (límite duro de reintentos por contenido idéntico) es la Tarea 3 del plan
> `docs/superpowers/plans/2026-09-25-el-arnes-cuenta-su-estado.md`, cerrada el 25/09/2026: la
> colapsa un historial de llamadas recientes dentro del mismo encargo
> (`src/arnes/ciclo/repeticiones.rs`), sin gastar cupo. No cubre el caso exacto de la sesión 102 —ahí
> el modelo repetía la auditoría completa de un encargo distinto en cada pasada, no la misma llamada
> a herramienta dentro del mismo encargo—, pero si INC-N01 no vuelve a dispararse por la causa de
> arriba, tampoco haría falta.

**Gravedad:** alta. Ha bloqueado a Programator por completo dos veces (sesiones 99 y 102 de
NatureLand), en ambos casos sin que nadie añadiera trabajo nuevo — el disparador fue *archivar*
encargos ya cerrados.

**Qué pasa.** El arnés manda a Programator el canal completo (`.gestor/canal/` en NatureLand) en
cada sondeo, no solo el delta desde la última lectura. Cuando un encargo cerrado se archiva dentro
de la misma carpeta que se sondea (mover su texto a un histórico que sigue viviendo ahí dentro), el
fichero de destino crece, y ese crecimiento se lee como contenido nuevo en la siguiente pasada.

**Consecuencias medidas:**
- Sesión 99: el encargo 009, ya archivado, fue **reatendido por Programator** porque su texto seguía
  vivo dentro de la carpeta vigilada. Gastó una inferencia completa (varios minutos a ~18 tok/s) en
  trabajo ya cerrado.
- Sesión 102: archivar el encargo 020 hizo crecer `historico-encargos.md` en ~10 KB dentro de la
  carpeta vigilada. Programator entró en un bucle de **11 reintentos** de la misma auditoría ya
  cerrada, hasta que el arnés lo abortó por superar `max_herramientas_por_encargo = 12`.
- Antes de eso, en algún punto anterior, los ficheros del canal llegaron a sumar **~35.000 tokens**
  frente a los 32.768 de la ventana de Programator, y el servidor rechazó todas las peticiones en
  bucle hasta que se podó el canal a mano.

**Mitigación aplicada por el lado de NatureLand (no arregla la causa):** mover los históricos fuera
de la carpeta que el arnés sondea (`.gestor/canal-historico/`, sesión 102). Funciona, pero es un
parche de convención humana, no una garantía del propio Programator: si alguien olvida la
convención, el problema vuelve.

**Recomendación para Programator:**
1. **Distinguir contenido nuevo de contenido movido/re-escrito.** Si el mecanismo de detección de
   «novedad» se basa en el tamaño en bytes de los ficheros sondeados (según se documenta también en
   `evaluacion/INCIDENCIAS.md`, INC-01, sobre `sesion.rs`), cualquier operación que reordene o mueva
   texto ya visto dentro del árbol vigilado se puede confundir con contenido nuevo. Un hash por
   sección (por encabezado `## Para Programator: ...`) en vez de un offset de bytes por fichero
   sería más robusto: lo que ya se hasheó y se marcó atendido no vuelve a atenderse aunque cambie de
   posición o el fichero que lo contiene crezca por otro motivo.
2. **Límite duro de reintentos por contenido idéntico o muy similar.** El límite actual
   (`max_herramientas_por_encargo = 12`) evita el cuelgue infinito, pero permite gastar 11
   inferencias completas en el mismo error antes de abortar. Detectar que las últimas N propuestas
   son casi idénticas (mismo nombre de fichero de salida, contenido con alta similitud) y abortar
   antes ahorraría tiempo real, que es el recurso más caro con un modelo a 18 tok/s.

---

## INC-N02 — Ningún aviso explícito cuando Programator agota su generación sin invocar ninguna herramienta de entrega

**Gravedad:** media. No bloquea nada, pero hace perder tiempo de diagnóstico: parece que Programator
no ha recibido el encargo, cuando en realidad lo procesó y no llegó a entregar nada.

**Qué pasa.** Si se le pide leer un fichero grande sin acotar (ver INC-N03) o, en general, si la
generación se le llena de contenido antes de llegar a invocar `publicar` o `escribir_propuesta`, el
resultado es **silencio total**: no hay entrada nueva en `programator.md`, no hay fichero nuevo en
`candidatos/`, y `latido.json` sigue diciendo `"estado": "reposo"` como si nada hubiera pasado.

**Evidencia concreta (NatureLand, sesión 101):** se le pidió `leer_fichero` sobre un fichero de
67.423 bytes / 1.366 líneas. Más de 30 minutos después, sin ninguna entrega. La consola del propio
arnés sí mostraba un ciclo de generación completo (`n_tokens = 24756, truncated = 0`) y un resumen
`Encargos atendidos: 1 (fallos al publicar: 0)` — es decir, el arnés **sabe** que generó una
respuesta larga y no truncada sin invocar ninguna herramienta, pero no lo reporta como un fallo en
ningún fichero que el equipo consulte normalmente (solo en la consola en vivo, que nadie deja
abierta permanentemente).

**Recomendación:** cuando una generación termina sin ninguna llamada a herramienta de entrega
(`publicar` o `escribir_propuesta`), escribir una línea explícita en `programator.md` (o en
`latido.json`) del tipo: *«Encargo detectado y procesado, pero no se invocó ninguna herramienta de
entrega — posible agotamiento del presupuesto de generación por contexto demasiado grande.»* Eso
convertiría 30 minutos de duda en una comprobación de 10 segundos.

---

## INC-N03 — `leer_fichero` no admite acotar un rango de líneas

**Gravedad:** media. Es la causa raíz de INC-N02 en al menos un caso documentado.

**Qué pasa.** La única forma de leer un fichero del proyecto es entero. Si el fichero es grande
(decenas de KB), pedirle que lo lea para localizar un fragmento concreto le llena la ventana de
contexto de código irrelevante antes de que pueda razonar sobre lo que se le pidió.

**Mitigación actual (de quien encarga, no de Programator):** si el fichero pesa más de unas pocas
KB, se le pega el fragmento exacto directamente en el encargo en vez de mandarlo a leerlo. Funciona,
pero traslada el trabajo de acotar a quien encarga, en cada ocasión, y solo si se acuerda de que el
límite existe (no se acordó la primera vez, ver INC-N02).

**Recomendación:** añadir parámetros opcionales `desde_linea` / `hasta_linea` a `leer_fichero`, o
que el arnés trunque automáticamente con un aviso (`«fichero de N líneas, mostrando las primeras
M»`) en vez de servir el fichero completo sin más. Alternativamente, un límite duro de tamaño con
mensaje de error explícito («este fichero excede lo que puedo leer de una vez, pide un fragmento»)
sería estrictamente mejor que el fallo silencioso actual.

---

## INC-N04 — Error de categoría recurrente y sistemático en auditorías SOLID: confunde usar campos/métodos propios de la misma clase con una violación de DIP

> **Resuelto en parte (Programator, commit `76b8c70`, 2026-10-01).** Recomendación aplicada:
> `plantillas/auditoria-solid.md`, con un ejemplo correcto y uno incorrecto de cada principio
> SOLID, el falso positivo de DIP marcado primero por ser el que más veces ha costado un encargo.
> Se referencia desde la guía de encargo. No arregla la comprensión del modelo —sigue sin «saber»
> SOLID— y es de inclusión manual (quien encarga la pega junto al encargo), decisión explícita del
> Director para no aplicar un recordatorio siempre activo a encargos donde no venga a cuento, el
> mismo problema que ya señalaba INC-N12 punto 4 sobre `recordatorio-de-encargo.md`.

**Gravedad:** media-alta para la utilidad de sus auditorías SOLID en concreto (no afecta a su buen
desempeño detectando números sueltos o métodos largos, que es donde de verdad rinde).

**Qué pasa.** En al menos **tres encargos distintos** de NatureLand, Programator ha marcado como
«violación de Inversión de Dependencias» que un método:
- llame a `GetTree().Root.FindChild(...)` (patrón ya establecido en 29 sitios de 7 ficheros del
  proyecto — encargo de auditoría de `GestorConstruccionRTS.cs`, sesión 101);
- llame a un método **estático propio** de la misma clase (auditoría de
  `VistaAgentes.Pelajes.cs`, sesión 101);
- use **campos propios** de su misma clase, como `_mundo` o `_btnPobladoActual` (auditoría de
  `HUDContextual.Poblados.cs`, sesión 102).

**Por qué importa que sea el MISMO error tres veces:** según la propia guía de uso de Programator en
NatureLand (`COMO-ENCARGAR-A-PROGRAMATOR.md`), sus fallos se dividen en dos familias con soluciones
distintas: los de **atención** (verificar dónde está un valor) se desactivan nombrándolos en el
encargo; los de **comprensión** (entender qué significa una regla) no. Este es de comprensión: se le
ha avisado explícitamente en encargos posteriores («no marques como DIP el uso de campos/métodos
propios») y aun así ha reaparecido con una variación distinta cada vez, lo que indica que el aviso
puntual por encargo no basta — no tiene fijado el concepto, solo evita repetir el ejemplo exacto que
se le señaló la vez anterior.

**Recomendación:** si Programator va a seguir usándose para auditorías con vocabulario de principios
de diseño (SOLID, DIP, etc.), convendría que el arnés pudiera inyectar automáticamente, para
encargos de ese tipo, una definición de referencia con un ejemplo correcto y uno incorrecto de cada
principio invocado — no como algo que cada persona que encarga tenga que redactar de cero cada vez,
sino como una plantilla reutilizable del propio Programator (algo como un
`plantillas/auditoria-solid.md` que el arnés ofrezca u obligue a incluir). Esto no arregla la
comprensión del modelo, pero sí evita que cada proyecto tenga que redescubrir y repetir por escrito
la misma corrección una y otra vez.

---

## INC-N05 — No hay memoria entre encargos: cada aviso aprendido hay que repetirlo a mano, en cada encargo, para siempre

> **Resuelto en parte (Programator, commit `76b8c70`, 2026-10-01).** El Director distinguió dos
> casos que esta incidencia mezclaba. Los fallos genéricos de Programator (válidos en cualquier
> proyecto) siguen el cauce que ya existía y no necesitaban mecanismo nuevo: se reportan en un
> documento como este, se revisan y se arreglan en una release, como las dos incidencias
> siguientes de esta misma tanda. El conocimiento específico de un proyecto (el ejemplo real de
> esta incidencia, el patrón ya establecido de NatureLand) tiene ahora una sección fija —«7.
> Lecciones aprendidas en este proyecto»— en `plantillas/instrucciones-del-proyecto.md`, que ya se
> incluye siempre: no hace falta un fichero nuevo con su propia regla de poda.

**Gravedad:** baja como defecto (es una decisión de diseño deliberada de confinamiento, documentada
y correcta desde el punto de vista de seguridad), pero **alta como fricción operativa** a medida que
el número de lecciones aprendidas crece.

**Qué pasa.** Programator no lleva memoria entre encargos por diseño. Es razonable y probablemente
deseable (evita que arrastre un contexto corrupto o unas conclusiones erróneas de sesión en sesión),
pero tiene un coste: cada vez que se detecta un fallo suyo recurrente (ver INC-N04, o el error de
ubicación al citar valores, o alucinar hardcodeos que no lo son), la única forma de mitigarlo es que
la persona que redacta el SIGUIENTE encargo se acuerde de escribir el aviso a mano, copiándolo de la
guía. A fecha de esta nota, el encargo tipo-auditoría en NatureLand ya incluye habitualmente 4-5
avisos de este tipo, y la lista solo puede crecer.

**Recomendación:** un mecanismo de «notas permanentes por proyecto» — un fichero que el arnés
siempre incluya en el contexto de Programator además del encargo del turno (por ejemplo,
`.gestor/canal/LECCIONES-PROGRAMATOR.md`, un fichero corto y acotado, no todo el histórico) sería
memoria de trabajo sin ser memoria de conversación: no recuerda lo que hizo, pero sí se le recuerdan
las reglas fijas del proyecto sin que haya que copiarlas a mano en cada encargo. Reduciría el riesgo
de que alguien olvide incluir un aviso ya conocido (como pasó, sin relación con Programator, con el
propio harness de Claude Code y el guard de aislamiento — ver el aviso de la sesión 103 en
`.gestor/incidencias-detectadas.md` de NatureLand: un aviso que ya se sabía no evitó tener que
resolverlo de nuevo por no estar en un sitio que se consultara automáticamente).

---

## INC-N06 — Sin comprobador configurado en NatureLand: toda propuesta de Programator se verifica a mano

> **Resuelto como documentación (Programator, commit `76b8c70`, 2026-10-01).** El mecanismo
> `[verificacion.comprobadores]` ya permitía apuntar a un comando propio; lo que faltaba era el
> ejemplo de cómo usarlo contra un proyecto con referencias cruzadas (Godot/.NET, Unity), que
> `programator.ejemplo.toml` no compila aislado. Documentado con un ejemplo de script envoltorio
> que copia la propuesta sobre una copia del proyecto real antes de compilar. Criterio del
> Director aplicado aquí también: lo que depende del proyecto anfitrión lo configura el proyecto
> anfitrión, Programator no intenta adivinarlo ni resolverlo por dentro.

**Gravedad:** media, es un problema de configuración del proyecto anfitrión más que de Programator
en sí, pero afecta directamente a cuánto se puede confiar en sus propuestas sin trabajo manual.

**Qué pasa.** El propio protocolo de Programator prevé que el arnés compile las propuestas que
entrega y añada una línea de veredicto (*"la hace el arnés, no el modelo"*). En el proyecto
NatureLand no hay ningún comprobador configurado para C#/.NET, así que esa línea nunca aparece con
un veredicto real: toda propuesta se copia a un contenedor Docker aparte (`mmcelt-linux`, con .NET
SDK 8) y se compila **a mano**, fuera del ciclo de Programator, antes de aplicarla.

**Recomendación (para quien mantiene Programator, no necesariamente para NatureLand):** si añadir un
comprobador de `dotnet build` contra un proyecto real (no solo el fichero suelto propuesto, porque
`escribir_propuesta` entrega ficheros sueltos que no compilan de forma aislada sin el resto del
proyecto) es viable dentro del propio arnés, cerraría el ciclo de verificación sin depender de un
paso manual externo. Si no es viable de forma genérica (cada proyecto anfitrión tiene su propio
build), al menos documentar con claridad en la plantilla de configuración (`programator.toml`) cómo
un proyecto anfitrión puede registrar su propio comando de verificación.

---

## INC-N07 — El encaje GPU/CPU no deja rastro histórico: no se puede saber si un reparto peor que otra vez es un cambio real o solo VRAM libre distinta en ese momento

> **Resuelto (Programator, commit `8c44150`, 2026-10-01).** `src/motor/encaje_historico.rs` añade
> una línea a `.gestor/<agente>/encaje-historico.log` en cada arranque, con marca de tiempo,
> versión, VRAM libre/total si hay GPU, contexto configurado y capas en GPU/total. Un fallo al
> escribir solo avisa, nunca aborta el arranque. También se investigó experimentalmente si el
> reparto de capas o la fiabilidad de una respuesta cambian entre un motor recién arrancado y uno
> que acaba de atender otro encargo (relacionado con el documento de detección de degradación): en
> dos pruebas controladas con semilla fija, la imprecisión de línea de INC-N08 se repitió igual en
> ambas condiciones, lo que no apoya que el estado del servidor sea la causa.

**Gravedad:** baja, es una mejora de transparencia más que un fallo funcional.

**Qué pasa.** Cada arranque de Programator calcula cuántas de las 40 capas caben en la VRAM libre en
ese instante (`caben 35 de 40 capas (piden 13.7 GiB)`, banner de arranque) y lo imprime una vez en
consola, pero no queda registrado en ningún fichero consultable después. Si en un arranque anterior
solo 1 capa iba a CPU y en el siguiente son 5, no hay forma de saber a posteriori si cambió algo real
(contexto configurado, versión de Programator, margen de seguridad) o si simplemente había más VRAM
libre en ese momento por otra razón externa (otro proceso usando la GPU). Se preguntó por este caso
concreto (sesión 103 de NatureLand, 2026-09-30) y no se pudo dar una respuesta certera — solo
hipótesis — precisamente por falta de este historial.

**Recomendación:** que el arnés anote cada cálculo de encaje (timestamp, VRAM libre detectada,
contexto configurado, capas GPU/CPU resultantes, versión de Programator) en un fichero de registro
propio y acumulativo — algo tan simple como una línea por arranque en
`.gestor/programator/encaje-historico.log` — para que comparar «hoy rinde peor que la semana pasada»
sea mirar dos líneas en vez de no poder saberlo.

## INC-N08 — Encargo 021 (auditoría de `HUDContextual.cs`): declaró haber cumplido una instrucción explícita que en realidad violó 14 de 24 veces, y CASI TODAS las líneas citadas son incorrectas

> **Resuelto en parte (Programator, commits `c01f6df` y `76b8c70`, 2026-10-01).** Recomendación
> nº 1 (no tratar la declaración de cumplimiento como señal de verificación): documentado en la
> guía de encargo, sin cambio de código posible —es una instrucción a quien verifica, no algo que
> el arnés pueda hacer cumplir—. Recomendación nº 2 (normalizar cada hallazgo buscando el
> fragmento literal citado y corregir la línea automáticamente): construido en
> `src/arnes/herramientas/auditoria.rs`, con el límite explícito de que un literal ambiguo (más de
> una coincidencia) o ausente se marca «sin verificar automáticamente» en vez de adivinar —ver
> `plantillas/formato-auditoria.md`—. Queda sin resolver, porque depende de que quien encarga pegue
> ese formato: una auditoría en prosa libre sigue sin ninguna garantía sobre la línea citada.

**Gravedad:** alta. Es el fallo más grave documentado hasta ahora en este fichero, porque combina
dos problemas nuevos que no estaban registrados en `COMO-ENCARGAR-A-PROGRAMATOR.md` ni en
evaluaciones previas: una **autoevaluación de cumplimiento falsa**, y una **imprecisión de
localización sistemática** (no un desliz aislado, sino en casi todo el fichero de hallazgos).

**Contexto.** Encargo 021, auditoría de `game/scripts/HUDContextual.cs` (345 líneas), con cinco
avisos explícitos, incluido: *«No toques ni menciones colores, tamaños de interfaz (por ejemplo
`Vector2(100f, 36f)` de un botón) ni texto visible como hardcodeo. Esos números son diseño visual
deliberado, no un defecto de código.»* Se pidió además, como sexto requisito, que **antes de
entregar escribiera una línea por cada aviso confirmando que lo había respetado**.

**Lo que entregó (`auditoria-hud-contextual.md`) al final del informe:**
> *«No propuse constantes para colores, `Vector2` de tamaño de control, ni texto de interfaz.»*

**Lo que de verdad hizo, verificado línea por línea contra el fichero real:** de sus 24 hallazgos de
«números sueltos», **14 son exactamente lo que se le dijo que NO hiciera**:
- 10 son valores dentro de `new Color(...)` (hallazgos 8, 9, 10, 11, 12, 15, 16, 18, 19, 20 de su
  informe) — colores de estilo, expresamente excluidos.
- 3 son anchura/radio de borde de un `StyleBoxFlat` (`SetBorderWidthAll`, `BorderWidthBottom`,
  `SetCornerRadiusAll`, hallazgos 13, 14, 17) — tamaños de interfaz, misma categoría de exclusión.
- 1 es, literalmente, **el mismo `Vector2(100f, 36f)` que el encargo citó como EJEMPLO EXPLÍCITO de
  lo que no había que tocar** (hallazgo 21). No es un caso parecido: es el caso exacto, con los
  mismos números, puestos en el encargo precisamente para que sirviera de ejemplo negativo.

Es decir: la frase de «cumplimiento» al final del informe es **falsa**, y no por omisión — el propio
informe, dos párrafos antes, contiene la prueba de que no se cumplió. Antes (ver INC-N04 y el propio
`COMO-ENCARGAR-A-PROGRAMATOR.md`) ya se sabía que declara haber seguido una regla sin comprenderla
del todo; esto es un paso más allá: **la declaración de cumplimiento en sí misma no es fiable**, ni
siquiera como señal de que el modelo «cree» haber cumplido — puede fallar y decir que no ha fallado
en el mismo documento.

**El segundo problema, independiente del primero — precisión de línea:** de los **24** hallazgos de
número suelto, se comprobaron las 24 líneas citadas contra el fichero real. **Ninguna coincide.**
El desplazamiento no es constante (lo que descartaría un error aritmético simple de "off-by-N"):
varía entre +4 y +26 líneas por encima de la línea real en la primera mitad del informe, y por
DEBAJO de la línea real (−18) en los últimos hallazgos, como si hacia el final del fichero perdiera
la cuenta en sentido contrario. Ejemplos:

| Hallazgo (su número) | Línea que dio | Línea real | Diferencia |
|---|---|---|---|
| 1 (`Layer = 95`) | 187 | 166 | +21 |
| 5 (`_tiempoToast < 0.6f`) | 229 | 209 | +20 (y además dijo 1 repetición cuando son 2: el valor `0.6f` también aparece en la línea 211) |
| 6 (`* 3.5f`) | 235 | 221 | +14 |
| 21 (`Vector2(100f, 36f)`) | 294 | 316 | −22 |
| 22 (volumen `0.7f`) | 300 | 318 | −18 |
| 24 (`_tiempoToast = 3.2f`) | 316 | 342 | −26 |

Los dos hallazgos de «método demasiado largo» (`_Process`, `ActualizarEstiloModo`) también dan
rangos de línea incorrectos, aunque ahí el nombre del método sí identifica la ubicación sin ambigüedad
(a diferencia de un número suelto, que sin la línea correcta hay que buscarlo a mano).

**Lo que SÍ acertó, y por qué vale la pena decirlo también:** de los 10 hallazgos que no violaban
ninguna exclusión, **7 señalan un número real y con valor** (aunque en la línea equivocada): el
`Layer = 95`, el umbral de fundido `0.6f`, la velocidad de pulso `3.5f`, la fórmula de amplitud de
alfa (`0.35f`/`0.15f`), y los dos volúmenes de sonido `0.7f`/`0.4f` y `3,2f` de duración del toast.
Todos aplicados directamente tras esta verificación (ver `.gestor/cola-priorizada.md`, NatureLand,
punto de la sesión 103). **También se le pasó por alto un volumen real** que sí encaja en la misma
categoría que él mismo detectó en otros sitios: `AudioSfxUi.Reproducir(..., 0.8f)` en `ConmutarModo`
(línea 236) — un volumen suelto, del mismo tipo exacto que sus hallazgos 22 y 23, que no detectó.

**Recomendación:**
1. **No tratar la sección de «cumplimiento de avisos» que el propio modelo escribe como una señal de
   verificación.** Este caso demuestra que puede ser categóricamente falsa incluso cuando la prueba
   de lo contrario está en el mismo documento, a pocas líneas de distancia. Cualquier automatización
   futura que se apoye en esa sección para decidir si aplicar algo sin revisión humana sería
   insegura contra este modelo.
2. **La precisión de línea de Programator en ficheros de ~300+ líneas no es fiable en absoluto**,
   no ya «con desliz ocasional» (como ya constaba en la guía por errores de ubicación puntuales) sino
   sistemáticamente, en prácticamente el 100% de sus citas de este encargo. Si el arnés pudiera
   **normalizar cada hallazgo buscando el fragmento literal citado en el fichero real** y corregir la
   línea automáticamente (en vez de confiar en el número que da el modelo), la utilidad de sus
   auditorías aumentaría mucho sin que el modelo tenga que mejorar en absoluto — es un problema de
   presentación de resultados, no solo de comprensión del modelo.
3. Seguir incluyendo ejemplos explícitos de lo que NO hacer en el encargo (como el
   `Vector2(100f, 36f)`) sigue siendo buena práctica — sin él, probablemente habría marcado aún más
   tamaños de interfaz como hardcodeo — pero este caso muestra que ni siquiera un ejemplo verbatim,
   puesto ex profeso, garantiza que no se repita exactamente.

## INC-N09 — La propia bandeja de Programator (`programator.md`) no tiene regla de poda documentada en ningún sitio, y por eso nadie la aplicaba

> **Resuelto (Programator, commit `9acb96f`, 30/09/2026).** Exactamente la recomendación nº 2:
> `Canal::rotar_a_historico` (`src/protocolo/canal.rs`) rota automáticamente, al inicio de cada
> pasada con encargos, todo el contenido anterior del buzón propio a
> `.gestor/canal/historico-{agente}.md`, dejando en el buzón vivo únicamente el último latido. No
> hace falta que ningún proyecto anfitrión recuerde la disciplina de poda: la aplica el arnés solo,
> en cada pasada, sin que nadie tenga que pedirlo por encargo.

**Gravedad:** media-alta. Es la misma familia de problema que INC-N01, pero en un sitio distinto y
más fácil de pasar por alto: no en el histórico del canal (que sí tiene regla, y se rompía), sino en
el propio fichero donde EL MODELO escribe sus latidos y entregas.

**Qué pasa.** `.gestor/canal/claude.md` (la bandeja de quien encarga) y `.gestor/canal/estado.md`
(hechos comprobables) tienen ambos una norma explícita y ya aplicada: «aquí queda sólo lo vivo»,
con sus respuestas cerradas archivadas fuera de la carpeta que el arnés sondea. **La bandeja de
Programator (`programator.md`) no tenía esa norma en ningún sitio** — ni en
`COMO-ENCARGAR-A-PROGRAMATOR.md`, ni en la documentación de este mismo repositorio (`docs/`,
`README.md`, `CLAUDE.md`), ni en los documentos de diseño/spec del arnés
(`docs/superpowers/specs/` y `/plans/`). Se comprobó expresamente antes de escribir esta entrada:
ninguna búsqueda por «podar», «archivar», «histórico» o «solo lo vivo» cerca de `programator.md` dio
resultado.

**Consecuencia medida.** Nadie podaba esa bandeja porque nadie sabía que hiciera falta. A fecha
2026-09-30 llevaba desde el 2026-09-23 sin tocarse, acumulando las respuestas de los encargos 006 a
021 ya cerrados: **20.991 bytes** que viajaban enteros en cada petición a Programator sin aportar
nada — el mismo mecanismo exacto de INC-N01, aplicado a un fichero distinto que se había quedado
fuera del radar. Fue una pregunta directa del Director («el canal entre tú y Programator solo
debería tener la tarea en curso, ¿no?») la que destapó el hueco: la intención de diseño (solo la
tarea activa) existe y es correcta, pero solo estaba **documentada y aplicada** para la mitad del
canal.

**Corrección aplicada esta vez** (por el lado de NatureLand, no de Programator): archivado el
contenido íntegro de `programator.md` a `../canal-historico/historico-actas.md`, y la bandeja vuelta
a su estado mínimo (`_Sin respuestas todavía._`), con una nota explicando la norma para que no se
repita el olvido.

**Recomendación para Programator:**
1. **Documentar explícitamente, en `COMO-ENCARGAR-A-PROGRAMATOR.md` o donde corresponda, que
   `programator.md` (o el equivalente en cualquier proyecto anfitrión) necesita la MISMA disciplina
   de poda que la bandeja de quien encarga.** No basta con que la convención exista para un lado del
   canal: si el arnés manda la carpeta entera, las dos bandejas —la de quien encarga y la del propio
   Programator— están sujetas exactamente al mismo problema.
2. **Mejor aún: que el arnés lo haga solo.** Si al cerrar/archivar un encargo el propio arnés pudiera
   truncar automáticamente su propia bandeja de respuestas ya resueltas (o simplemente no reenviar
   turnos de conversación anteriores al último encargo activo), ningún proyecto anfitrión tendría que
   acordarse de esta disciplina manual — que ya ha demostrado, dos veces sobre dos bandejas distintas,
   que se olvida si no está escrita en algún sitio.

## INC-N10 — Encargo 025 (auditoría de `PantallaMapaCampania.cs`): formato de respuesta incumplido de raíz y señaló exactamente los colores que el encargo prohibía nombrar

> **Investigado y resuelto en parte (Programator, commits `c01f6df` y `8c44150`, 2026-10-01).** La
> recomendación (rechazar un formato inválido y pedir reformulación) se descartó tal cual estaba
> planteada: el formato de línea no estaba definido por Programator en ningún sitio, así que
> rechazar contra un formato ajeno habría sido frágil. En su lugar se formalizó el formato
> (`plantillas/formato-auditoria.md`) con verificación de línea —ver INC-N08—, que cubre la mitad
> de este caso (la línea citada) pero no la otra (que este encargo en concreto señaló
> exactamente los colores excluidos, un fallo de comprensión del encargo, no de formato). Se
> reprodujo el experimento de dos condiciones (motor recién arrancado / motor que acababa de
> atender otro encargo) sobre el mismo fichero exacto de este caso, con semilla fija: en ninguna de
> las dos se repitió el colapso grave (bloque único, contenido duplicado, colores excluidos
> incluidos), lo que no permite ni confirmar ni descartar el espaciado temporal como causa con una
> sola muestra por condición — ver el documento de detección de degradación para el detalle.

**Lo que pasó.** El encargo (sesión 104) pedía, con el mismo formato ya usado con éxito en los
encargos 021-024, una línea por hallazgo (`TIPO | miembro | qué pasa | qué harías`), con número de
línea real citado, y avisaba explícitamente de 12 constantes/colores ya nombrados en la cabecera
del fichero que **no** debían señalarse (`ColorRegionActiva`, `ColorOceanoProfundo`,
`ColorOceanoBajio`, `ColorBordeRegion`, `ColorCosta`, `ColorContornoCosta`, `ColorRio`, y 5
constantes numéricas más).

La respuesta no dio ninguna línea de fichero en absoluto (incumple la regla obligatoria de citar
línea real) y volcó el hallazgo `HARDCODEO` como una sola línea con más de 60 números sueltos
separados por comas, sin miembro, sin ubicación, sin poder saber a qué literal concreto del fichero
corresponde cada uno. **Verificado que la lista incluye, literalmente, los 7 colores que el encargo
prohibía señalar**: `0.95, 0.85, 0.25` (=`ColorRegionActiva`), `0.05, 0.12, 0.22`
(=`ColorOceanoProfundo`), `0.1, 0.22, 0.34` (=`ColorOceanoBajio`), `0.08, 0.06, 0.04`
(=`ColorBordeRegion`), `0.75, 0.68, 0.45` (=`ColorCosta`), `0.96, 0.94, 0.85`
(=`ColorContornoCosta`), `0.3, 0.55, 0.85` (=`ColorRio`) — y además la lista entera aparece
**duplicada dentro de sí misma** (el mismo tramo de ~40 números se repite dos veces seguidas en la
misma línea de respuesta), lo que sugiere que se concatenó el mismo barrido del fichero dos veces
sin deduplicar antes de entregar.

**Mismo patrón que INC-N08** (encargo 021: declaró cumplir una instrucción explícita que en
realidad violó 14 de 24 veces), pero esta vez sin siquiera declarar cumplimiento — directamente no
seleccionó lo pedido. A diferencia de encargos 022-024 (8/9, 9/12, 6/6 con formato correcto), este
es un retroceso completo a un formato inservible para verificar uno por uno.

**Consecuencia práctica en NatureLand.** El hallazgo `HARDCODEO` se descartó entero (no verificable
línea por línea, formato incumplido, viola la exclusión explícita). Solo sobrevivieron, verificados
a mano contra el fichero real, el `DUPLICADO` (cuatro botones de pestaña con el mismo patrón de
construcción, real y aplicado con un helper `AnadirBotonPestana`) y el juicio general de `SOLID`
(clase con varias responsabilidades, aceptado como diagnóstico razonable pero de alcance
arquitectónico, no aplicado). El `TAMAÑO` de `_Ready()` se consideró débil (ya delega en varios
métodos extraídos) y no se aplicó.

**Recomendación:** si el arnés puede detectar que una respuesta no sigue el formato de línea
pedido (por ejemplo, contando líneas con el separador `|` esperado), rechazarla y pedir
reformulación antes de entregarla como candidata — ahorraría el turno de verificación manual en
casos como este, donde el 90% del contenido entregado no es utilizable tal cual.

## INC-N11 — El modelo responde «[solicito leer_fichero]» como TEXTO, imitando el rótulo que el propio arnés escribe en el historial, y el encargo muere sin entrega

> **Resuelto (Programator, commit `97f1c7b`, 2026-10-01).** `src/arnes/ciclo.rs` ya detecta que una
> `Respuesta::Texto` empieza por el mismo prefijo (`[solicito `) con el que el arnés etiqueta una
> llamada resuelta en el historial: exactamente la recomendación nº 1. En vez de cerrar el encargo
> en el acto, avisa al modelo de que ha escrito la petición como texto plano y le deja rectificar,
> dentro del mismo cupo de denegaciones seguidas que ya protegía contra la insistencia tras una
> denegación (`max_denegaciones_seguidas`); si insiste, aborta con un motivo explícito en vez de
> terminar en silencio con `SinEntrega`. La recomendación nº 2 (representar las llamadas con el
> formato nativo de *tool calls* de la plantilla de chat, no como texto plano) sigue sin hacerse: es
> un cambio en la integración con `llama-server` que no entra en esta tanda, y el motivo para no
> necesitarla con la misma urgencia es que el síntoma que causaba ya no pierde el encargo.

> Detectado por: **Claude, Agente 1** (principal) de NatureLand · **2026-10-01, 13:40** · sesión 107.

**Gravedad:** alta. Pierde el encargo entero sin ningún error del motor, y el síntoma engaña: parece
que el modelo «pidió» una herramienta, cuando en realidad no la llamó.

**Circunstancia exacta.** Encargo 027 de NatureLand (auditoría de números sueltos en los tramos nuevos
de `HUDContextual.cs` y `HUDContextual.Vistas.cs`). El encargo pedía **cuatro lecturas acotadas** con
`leer_fichero` (`desde_linea`/`hasta_linea`, unas 170 líneas en total) más una quinta para las
constantes existentes. `latido.json` pasó a `atendiendo` a las 13:39:57 y volvió a `reposo` a las
13:40:38 (unos 40 s, registrado por un monitor que leía el latido cada 5 s). En `programator.md` el
arnés anotó: «⚠️ Programator no invocó herramientas de entrega», y como «Respuesta directa del
modelo» solo: `[solicito leer_fichero]`. Ningún fichero nuevo en `candidatos/`.

**Causa, comprobada en el código del arnés** (no es una hipótesis sobre el modelo):
- `src/arnes/ciclo.rs:110` y `:162`: cada llamada a herramienta se guarda en la conversación como un
  mensaje del **modelo** cuyo contenido es el texto literal `format!("[solicito {nombre}]")`.
- `src/arnes/ciclo.rs:94-98`: si el modelo devuelve `Respuesta::Texto`, el bucle termina en el acto con
  `Desenlace::SinEntrega(texto)`.
- Así que, después de una o más lecturas reales, el historial que ve el modelo contiene turnos suyos
  que **son exactamente** `[solicito leer_fichero]`. Un modelo de 24B imita ese patrón: escribe el
  rótulo como texto plano en lugar de emitir la llamada estructurada, y el arnés lo toma por respuesta
  final. Que el texto devuelto coincida **carácter por carácter** con el formato del rótulo del arnés
  es la evidencia.
- **No es un problema de tamaño.** La incidencia equivalente de la sesión 101 (ver INC-N02) se atribuyó
  a leer un fichero de 67 KB entero; aquí las lecturas eran acotadas. Probablemente aquel caso tuvo
  la misma causa, total o parcialmente.
- Pedir **varias lecturas** en un mismo encargo multiplica los rótulos en el historial y, con ellos, la
  probabilidad de imitación. Con una sola muestra en este fichero no se puede cuantificar.

**Mitigación aplicada desde NatureLand:** encargo 027-bis con los tramos pegados en el propio encargo,
numerados, y la instrucción explícita de no llamar a `leer_fichero`.

**Recomendaciones para el arnés (por orden de coste):**
1. Detectar el caso: si una `Respuesta::Texto` empieza por `[solicito ` (el formato propio del arnés),
   no cerrar el encargo; devolver al modelo un aviso del tipo «Has escrito la petición como texto; para
   usar una herramienta, invócala» y seguir el bucle (con el cupo de denegaciones habitual).
2. Representar en el historial las llamadas a herramienta con el formato nativo de *tool calls* de la
   plantilla de chat del modelo, en vez de un texto plano que el modelo puede copiar.
3. Mientras tanto, documentar en `COMO-ENCARGAR-A-PROGRAMATOR.md` que pedir varias lecturas en un mismo
   encargo es arriesgado, y que pegar el fragmento es la vía fiable.

**Nota de actualización sobre INC-N02 e INC-N03** (mismo autor y fecha): el arnés ya escribe en
`programator.md` un aviso explícito cuando no hay entrega, y `leer_fichero` ya acepta
`desde_linea`/`hasta_linea`. Las dos recomendaciones están aplicadas; este caso es el primero
observado después de aplicarlas, y muestra que el fallo de fondo era otro.

---

## INC-N12 — La guía para quien encarga (`plantillas/como-encargar-a-programator.md`) y las plantillas que recibe el modelo: dónde se contradicen con el uso real y qué mejorar

> Detectado por: **Claude, Agente 1** (principal) de NatureLand · **2026-10-01** · sesión 107.
> Revisadas: `plantillas/como-encargar-a-programator.md` (la que se publica como
> `.gestor/canal/COMO-ENCARGAR-A-PROGRAMATOR.md`), `preambulo-del-modelo.md`,
> `instrucciones-del-proyecto.md` y `recordatorio-de-encargo.md`, contrastadas con los encargos
> 001-027 de NatureLand y con `src/arnes/`.

**Gravedad:** media-alta. La guía es lo primero que lee un modelo de pago (Claude, Gemini, Codex)
antes de encargar. Donde contradice lo medido, induce a repetir fallos que ya costaron encargos.

### 1. Recomienda justo lo que más encargos ha perdido: «Aprovecha `leer_fichero`»
La guía dice: «en vez de pegarle un fichero de 19 KB dentro del encargo, dale la ruta y pídele que lo
lea». En NatureLand, los dos encargos perdidos sin entrega (sesión 101 e INC-N11) fueron
precisamente encargos que le pedían leer. Los que llevaban el código pegado entregaron. Hoy, el
mismo encargo con los tramos pegados y **numerados** (027-bis) entregó con 24 de 25 líneas bien
citadas, mientras que en los encargos con lectura las líneas salían desplazadas (INC-N08, encargo 026).
**Mejora:** invertir la recomendación. Pegar el fragmento exacto, con su número de línea real
delante, es la vía fiable. `leer_fichero` queda para consultas puntuales («¿existe ya esta
constante?»), una por encargo, mientras no se arregle INC-N11.

### 2. Mezcla de destinatario: partes escritas para el modelo dentro de la guía para quien encarga
«Tienes **5 herramientas**», «Si llamas a una herramienta sin un argumento…», «si insistes tras una
denegación, el encargo se aborta» y «Revísala tú con más cuidado antes de publicarla» hablan al
propio Programator, no a quien encarga. Quien lee la guía tiene que adivinar a quién va cada frase.
**Mejora:** guía en tercera persona («Programator tiene 5 herramientas…»), y las normas para el
modelo solo en el preámbulo.

### 3. Las plantillas no coinciden entre sí en el repertorio ni en el protocolo de entrega
- `instrucciones-del-proyecto.md` §2 enumera **3** verbos (leer, escribir propuesta, publicar). La
  guía y `src/arnes/herramientas.rs:211-220` tienen **5** (también `listar` y `reservar`).
- `instrucciones-del-proyecto.md` §3 dice «escribe la propuesta y **después llama a `publicar` una
  sola vez**» con la declaración por requisito, pero los encargos que la guía enseña dicen «entrégalo
  con `escribir_propuesta`», y en la práctica es el arnés quien publica «He dejado la propuesta…». Con
  esto, la declaración por requisito acaba dentro de la propuesta o en ningún sitio.
**Mejora:** una sola fuente del repertorio, generada desde el código (`{repertorio}` ya existe en el
preámbulo; usarlo también en las instrucciones), y un único protocolo de entrega descrito igual en
las tres plantillas.

### 4. El recordatorio de código se pega a TODOS los encargos, también a las auditorías
`recordatorio-de-encargo.md` está activo por defecto (`src/config.rs`, `recordatorio_por_defecto`) y
se añade a cada encargo. Termina con «escribe **una línea por cada punto**» de las normas de código
(bordes, español, separar el cálculo…). En una auditoría de solo lectura eso son siete requisitos
ajenos que compiten con los del encargo, en un modelo que ya mezcla instrucciones (INC-N08, INC-N10:
declaró cumplir exclusiones que violó). Hoy volvió a incluir literales que el encargo excluía
expresamente (`1.0f`, `0f`, valores dentro de `new Color`).
**Mejora:** aplicar el recordatorio solo cuando el encargo pide código, o permitir desactivarlo por
encargo (p. ej. una marca en el encabezado).

### 5. Faltan en la guía lecciones que hoy solo conoce quien ha sufrido el fallo
Están en `.gestor/ESTADO.md` de NatureLand o en este fichero, no en la guía que lee un modelo nuevo:
- **El canal viaja entero en cada sondeo** (INC-N01): archivar fuera de `.gestor/canal/`, no pegar
  ficheros enormes, presupuesto aproximado frente a la ventana de 32 768 tokens.
- **Firmas de fallo y qué hacer:** «⚠️ no invocó herramientas de entrega» con `[solicito X]` como
  respuesta → reintentar con encabezado nuevo y el código pegado (INC-N11); bucle de reintentos sobre
  trabajo cerrado → el canal ha crecido (INC-N01).
- **Falsos positivos recurrentes**, para que quien verifica sepa dónde mirar: confundir el acceso a
  campos o métodos propios con una violación de DIP (INC-N04); proponer inyectar clases estáticas o
  tipos de datos; proponer constantes nuevas para un valor que ya tiene nombre, aunque él mismo lo
  note (encargo 027-bis: `0.4f` frente a `VolumenHoverBoton`).
- **El latido:** la guía menciona `.gestor/<agente>/latido.json`, pero no dice qué significan
  `reposo`/`atendiendo`/`proximo_sondeo` ni que basta vigilar sus transiciones para saber cuándo
  terminó un encargo, que es como se ha detectado cada fallo hoy.

### 6. La tabla de desempeño está congelada desde el 24/09 con 14 encargos
NatureLand lleva 27. Desde entonces hubo dos retrocesos de formato (INC-N08, INC-N10), una
recuperación (026: 10 de 13) y hoy 19 de 25 en una auditoría de formas con el código pegado. La propia guía
exige cifra y fecha por fila, pero no hay mecanismo para incorporar mediciones nuevas, así que sus
cifras envejecen sin aviso.
**Mejora:** fila por clase de encargo con «última medida» y número de muestras, y anotar si el código iba
pegado o se leía, porque es la variable que más ha pesado.

### 7. «No hay comprobador configurado» sin decir cómo configurarlo
La guía constata que en NatureLand nadie compila las propuestas, pero no explica cómo configurar un
comprobador (p. ej. para `.cs`). En NatureLand ya existe un contenedor con .NET SDK que podría
hacerlo (`mmcelt-linux`).
**Mejora:** una sección breve «Cómo configurar un comprobador para tu lenguaje», con un ejemplo.

---

## INC-N13 — El mismo encargo (027-bis) se atendió dos veces seguidas: la segunda, sin entrega y peor que la primera

> **Resuelto (Programator, commit `97f1c7b`, 2026-10-01), recomendación nº 1.** La hipótesis
> apuntaba a `src/arnes/sesion.rs:337-342`: si falla la publicación del **cierre** del encargo, el
> registro no avanzaba aunque la propuesta ya hubiera quedado anotada en el canal al entregarse
> (`Canal::publicar_entrega`, incidencia C2). Ahora `fallo_de_cierre_pierde_el_encargo`
> (`src/arnes/sesion/publicacion.rs`) distingue los dos casos: si el encargo entregó al menos una
> propuesta y no queda ningún veredicto sin publicar, el fallo del cierre no bloquea el avance del
> registro y el encargo no se reatiende; solo lo bloquea cuando de verdad no quedó ningún rastro en
> el canal. Las recomendaciones nº 2 (marcar en el buzón que una respuesta es un reintento) y nº 3
> (volcar el motivo del fallo en `latido.json`) siguen sin hacerse: no hacían falta para que el
> síntoma concreto —reatender un encargo ya resuelto— dejara de producirse, y añadirlas sin un caso
> que las necesite habría sido construir para una hipótesis, no para lo medido.

> Detectado por: **Claude, Agente 1** (principal) de NatureLand · **2026-10-01, 13:49** · sesión 107.

**Gravedad:** media. No se pierde trabajo, pero se gasta una inferencia completa (~3 min de GPU) y se
deja en el buzón una segunda respuesta que contradice a la primera y confunde a quien verifica.

**Circunstancia exacta** (transiciones de `latido.json` registradas cada 5 s):
- 13:43:36 `atendiendo` encargo 027-bis → ~13:46 entrega correcta con `escribir_propuesta`
  (`auditoria-hud-iconos-fade.md`, 25 hallazgos, 19 ciertos). En el buzón solo aparece la entrada
  «He dejado la propuesta…» con su veredicto; **falta el cuerpo de cierre del desenlace**, que en encargos
  anteriores (p. ej. 026) sí aparecía como segunda entrada.
- 13:46:57 `atendiendo` **otra vez el mismo encargo**. Nadie había tocado `claude.md`: el archivado del
  encargo por parte de NatureLand fue a las 13:48:13, con este segundo pase ya en curso.
- 13:49:44 `reposo`. Desenlace `SinEntrega`: respuesta en texto plano, sin herramienta, con 10
  hallazgos más que el primer pase, casi todos componentes RGB dentro de `new Color(...)`, que el
  encargo excluía expresamente.

**Causa más probable (hipótesis, no confirmada: falta la salida de consola del arnés):**
`src/arnes/sesion.rs:337-342`: si falla `canal.publicar` del desenlace, el registro de lectura no
avanza y el encargo se reatiende en el ciclo siguiente, por diseño («es preferible responder dos veces
que perder un encargo»). La ausencia del cuerpo de cierre en el buzón encaja con ese fallo de
publicación. Para confirmarlo hay que mirar la consola del arnés (mensaje
`mensaje_fallo_publicar_desenlace`) o `Encargos atendidos: N (fallos al publicar: M)`.

**Recomendaciones:**
1. Cuando el desenlace no se pueda publicar pero **sí hubo entrega** (propuesta escrita y su aviso
   publicado), avanzar el registro igualmente: el encargo ya no está perdido, y repetirlo solo
   produce una segunda versión contradictoria.
2. Si se reatiende un encargo, marcarlo en el buzón («⟳ reintento del encargo X por fallo al
   publicar») para que quien verifica sepa que la segunda respuesta no es un encargo nuevo.
3. Escribir el motivo del fallo de publicación en `latido.json` (`estado: "error"` con detalle), no
   solo en stderr.

---

## Nota de contexto: por qué se creó este fichero

El Director de NatureLand pidió (2026-09-30) que cualquier problema, fallo o posible mejora
detectado en Programator, en la comunicación con él, o en cualquier detalle de la relación entre
ambos proyectos, quede anotado en tiempo real en un fichero dedicado dentro de la carpeta del
proyecto Programator, para que sirva de insumo a quien lo desarrolle. Este fichero se irá
actualizando durante la sesión 103 de NatureLand (y sesiones futuras, si el mismo agente continúa)
a medida que se detecten casos nuevos — no es un informe cerrado de una sola vez.
