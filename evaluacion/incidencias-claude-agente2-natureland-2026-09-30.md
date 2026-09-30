# Incidencias y mejoras detectadas — Claude (Agente 2), desde el uso real en NatureLand

> Autor: Claude, Agente 2 (relevo) del proyecto NatureLand.
> Fecha de creación: 2026-09-30.
> Registro en caliente, actualizado en tiempo real mientras se detectan problemas, fallos o
> posibles mejoras en Programator, en la comunicación con él (el canal, el arnés, el protocolo de
> encargo/entrega) o en cualquier otro aspecto de la relación entre ambos proyectos. No sustituye a
> `evaluacion/INCIDENCIAS.md` (sesión de evaluación dedicada del 22/09/2026) ni a
> `evaluacion/desempeno-uso-real-natureland.md` (medición de desempeño con 14 encargos): este
> fichero recoge lo que se ha ido observando en el USO REAL de Programator dentro de NatureLand a lo
> largo de varias sesiones (98 a 103), desde el lado de quien lo dirige, no desde una sesión de
> pruebas dedicada.

---

## INC-N01 — El canal viaja ENTERO en cada pasada: cualquier crecimiento, aunque sea archivar trabajo ya cerrado, puede tirar la ventana de contexto

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

## Nota de contexto: por qué se creó este fichero

El Director de NatureLand pidió (2026-09-30) que cualquier problema, fallo o posible mejora
detectado en Programator, en la comunicación con él, o en cualquier detalle de la relación entre
ambos proyectos, quede anotado en tiempo real en un fichero dedicado dentro de la carpeta del
proyecto Programator, para que sirva de insumo a quien lo desarrolle. Este fichero se irá
actualizando durante la sesión 103 de NatureLand (y sesiones futuras, si el mismo agente continúa)
a medida que se detecten casos nuevos — no es un informe cerrado de una sola vez.
