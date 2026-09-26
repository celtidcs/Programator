# Desempeño en uso real — 14 encargos sobre un proyecto de verdad

**Devstral Small 2 24B a través de Programator, 22 a 24 de septiembre de 2026.**
Los encargos se le dieron por el canal, como se le darían en producción, y cada entrega se verificó
contra el código antes de aceptarla o rechazarla. Proyecto: **NatureLand** (Godot 4.7 + C#, unos 200
ficheros de código vivo).

## Qué aporta esto que no aporta el banco de pruebas

`informe-final.md` y los bancos de esta misma carpeta miden **capacidad**: encargos sintéticos,
semilla fija, pruebas adversarias preparadas, nota de 0 a 5. Este documento mide otra cosa:
**rendimiento en producción**, sobre un código que ya existía, con defectos reales que nadie había
sembrado y con un humano decidiendo si la entrega entra o no.

Las diferencias importan para optimizarlo:

- **Nadie preparó los ficheros.** Se le dieron tal cual estaban, con su ruido y su historia.
- **Todo se verificó contra el código**, hallazgo por hallazgo, con `grep` y contando líneas.
- **Hay coste de oportunidad**: cada encargo son 2-3 minutos de inferencia y 10-15 de verificación
  humana. Un encargo que no aporta no es neutro, resta.
- **Aparecen modos de fallo que un banco no puede provocar**, como duplicar una constante que ya
  existe en otro fichero del proyecto —imposible de detectar sin saber qué hay en el proyecto—.

Las actas completas, encargo por encargo, están en el repositorio de NatureLand:
`.gestor/canal/historico-actas.md`.

## 1. Los catorce encargos

| # | Qué se le pidió | Tipo | Resultado |
|---|---|---|---|
| 001 | `ContrasteGalon.cs` | escribir | ✅ 8/8 criterios. Usó `MathF.Min` fuera de lo acotado |
| 002 | `LegibilidadSobreFondo.cs` | escribir | ✅ 8/8. Namespace en bloque en vez del del proyecto |
| 003 | `RebordeSegunFondo.cs` | escribir | ✅ limpio. Respetó el requisito fácil de incumplir |
| 004 | `ComposicionAlfa.cs` | escribir | ✅ con corrección: **duplicó dos constantes** que ya existían en el dibujo |
| 005 | `PaletaHeraldica.cs` | escribir | ✅ con **3 correcciones**, una potencialmente grave (ver §4) |
| 006 | `DistanciaPerceptual.cs` | escribir (fórmula literal) | ⚠️ **3 entregas en 10 min**, fórmula sustituida y paréntesis mal repartido |
| 007 | `ContabilidadLluviaPorBioma.cs` | escribir | ✅ **14/14 requisitos**. Su mejor entrega de código |
| 008 | auditar `Tribu.cs` | auditar | ✅ **10 de 12** |
| 009 | por qué no se ve la lluvia | **diagnóstico causal** | ❌ **0 de 12** |
| 010 | auditar `PantallaEstadisticas.cs` | auditar | ⚠️ 9 de 14 |
| 011 | `HoraDelDia.cs` | escribir | ✅ **14/14 requisitos y 5/5 valores de referencia** |
| 012 | auditar `LluviaVisual.cs` | auditar | ✅ **6 de 6** |
| 013 | auditar `RelojSimulacion.cs` (recién escrito) | auditar | ❌ **0 útiles de 4** |
| 014 | auditar `GestorGuardado.cs` | auditar | ⚠️ **2 de 5** |

## 2. Lo que dicen los números, por tipo de tarea

| Tipo de tarea | Encargos | Balance |
|---|---|---|
| **Escribir una función acotada** | 8 | **8 aceptadas de 8** — pero **5 necesitaron corrección al integrar** |
| **Auditar código ajeno** | 5 | **27 hallazgos ciertos de 41** (66 %), con una varianza enorme: de 6/6 a 0/4 |
| **Diagnosticar una causa** | 1 | **0 de 12** |

**El dato que más engaña es el primero.** «8 de 8 aceptadas» suena a fiabilidad total, y no lo es:
sólo **3 de las 8** entraron sin tocar nada. Las otras cinco pedían trabajo de integración que no
aparece en la estadística de aceptación.

## 3. La variable que de verdad predice si acierta

No es el tipo de tarea: es **qué clase de pregunta se le hace sobre el código**.

| Clase de hallazgo | Qué exige | Cómo le sale |
|---|---|---|
| **De forma** — «hay un número suelto», «este método es largo», «esta clase hace seis cosas» | reconocer un patrón en el texto | **casi siempre acierta** |
| **De ubicación** — «ese número está en este método» | mirar otra vez dónde está | **acierta si se le pide expresamente** (14 de 14 tras advertírselo) |
| **Semántico** — «falta una validación», «no comprueba X», «podría pasar Y» | entender qué hace el código | **6 producidos, 0 accionables** |
| **Causal** — «por qué se comporta así» | descartar hipótesis contra evidencia | **falla siempre** (0 de 12) |

El encargo 014 lo enseña en una sola entrega: **sus dos hallazgos ciertos fueron de forma** —un `1`
incrustado y un método de 54 líneas— y **sus tres fallos, semánticos**: propuso validar la fecha de
guardado y los paquetes climáticos, dos cosas que el código ya valida en las líneas 73 y 236.

## 4. Los cinco modos de fallo, con su nombre y su antídoto

**1. Propone lo que el código ya hace.** Tres veces en dos encargos. En el 013 propuso «simplificar»
una condición dejándola **exactamente igual**. *Antídoto: ninguno conocido.* Se le advirtió por
escrito en el 014 y lo repitió dos veces. **Hay que verificarlo siempre.**

**2. Sustituye lo que se le da literal por algo equivalente que recuerda.** Encargo 006: se le dio la
fórmula `7.787f * t + 16f/116f` con la orden de no cambiarla, y escribió `KAPPA * t + 16f/116f` con
κ = 903,3. No inventó nada —κ es la constante real y 7,787 = 903,3/116— pero **repartió mal el
paréntesis** y el término quedó 116 veces mayor. *Antídoto: prohibirle constantes propias por escrito
**y** darle un valor de referencia calculado de antemano.* Con las dos cosas, el encargo 011 salió
perfecto.

**3. Duplica lo que ya existe en el proyecto.** Encargo 004: entregó `AlfaRelleno = 0.88f` y
`AlfaReborde = 0.85f` como constantes suyas, duplicando los literales que ya estaban en el dibujo.
Dos copias del mismo número acaban divergiendo sin que nada avise. *Causa: no lee del disco, así que
**no sabe qué hay en el proyecto**.* No es un descuido suyo: es una limitación del arnés. *Antídoto:
al integrar, buscar siempre si el valor ya tiene dueño.*

**4. Usa formas del lenguaje que rompen otra cosa.** Encargo 005: redeclaró las propiedades
posicionales de un `record struct`, convirtiéndolas de `init` a sólo lectura. Es **la forma exacta**
del defecto que esa misma mañana había hecho que el clima regional se guardara entero a cero.
*Antídoto: leer el código entregado, no sólo ejecutarlo.* Esto no lo caza una prueba.

**5. Rellena cuando no hay nada que encontrar.** Encargo 013: se le dio un fichero recién escrito y
cuidado, y en vez de decir «no veo defectos» produjo cuatro hallazgos, todos inútiles. **Su tasa de
acierto correlaciona con la densidad de defectos reales del fichero**: `Tribu.cs` (389 líneas, mucho
hardcodeo) → 10/12; `LluviaVisual.cs` (literales por todas partes) → 6/6; un fichero limpio → 0/4.
*Antídoto: dale ficheros que **sospechas** cargados, nunca ficheros que quieres **confirmar** limpios.*

## 5. Tabla de decisión: qué encargarle y qué no

| Tarea | ¿Encargársela? | Coste de verificar | Por qué |
|---|---|---|---|
| **Auditar un fichero que sospechas cargado de números sueltos y métodos largos** | **SÍ, es su mejor uso** | ~10 min: un `grep` por cada valor citado | 6/6 y 10/12 en los dos ficheros cargados |
| **Escribir una función pura acotada, con requisitos en lista y un valor de referencia** | **SÍ** | ~15 min: ejecutarla contra la referencia | 14/14 y 5/5 en el encargo 011 |
| **Escribir una estructura de datos clásica** (tabla, caché, parseador) | **SÍ, con revisión de formas del lenguaje** | ~15 min, leyendo el código | Acierta la estructura; los fallos son de integración |
| **Traducir o reescribir código siguiendo un patrón dado** | Sí, previsiblemente | medio | No medido aquí, pero es la misma tarea de reconocer formas |
| **Auditar buscando validaciones ausentes o errores semánticos** | **NO** | alto y con falsos positivos | De los **6** hallazgos semánticos que ha producido, **ninguno** resultó accionable |
| **Auditar un fichero limpio para confirmar que lo está** | **NO** | tiempo perdido | Rellena con hallazgos inventados (encargo 013) |
| **Diagnosticar la causa de un fallo** | **NO, nunca** | verificarlo cuesta más que medirlo tú | **0 de 12**, con aplomo |
| **Decidir arquitectura, aprobar, ejecutar, consultar APIs** | **NO** | — | No puede: el arnés lo impide, o alucina (ver §6) |

## 6. Lo que NO hay que preguntarle nunca, y por qué

**APIs, versiones y parámetros de bibliotecas.** Medido el 22/09: se le preguntó por tres funciones
inexistentes y sólo admitió que no existían en 4 de 9 intentos. `tokio::task::spawn_blocking_scoped`
se la inventó **3 de 3 veces**, con firma, garantías y ejemplo. **El riesgo no está en las preguntas
raras: está en las razonables** — alucina justo donde la comunidad tiene un hueco conocido, porque
ahí lo inventado *suena* a algo que debería existir. Y no puede comprobarlo: no navega, no tiene red
y no consulta documentación.

## 7. ¿Ayuda o es un lastre? El balance honesto

**Ayuda, en su banda, y de tres maneras concretas:**

1. **Trabaja en paralelo.** Mientras infiere sus dos o tres minutos, el agente que lo dirige sigue con
   lo suyo. Las auditorías de los encargos 012 y 014 salieron mientras se escribía y verificaba el
   reloj.
2. **Encuentra cosas ciertas que nadie había mirado.** El `1` incrustado de `IdTribu` lleva
   apareciendo en **tres** ficheros distintos y salió de dos auditorías suyas, no de una revisión
   humana.
3. **Incluso cuando falla, verificarlo paga.** Encargo 013: sus cuatro hallazgos eran inútiles, pero
   comprobar uno de ellos destapó un desbordamiento real que nadie había visto —convertir un `double`
   fuera de rango a `long` no lanza en C#, deja un valor indeterminado—. **Un hallazgo flojo en el
   sitio correcto vale más que ninguno.**

**Es un lastre si se le saca de ahí**, y el encargo 009 es el ejemplo caro: doce causas plausibles y
ninguna cierta, todas ignorando un dato que estaba **en el propio encargo**. Perdió una tarde y la
causa acabó saliendo de instrumentar y medir. **Si necesitas la causa de un fallo, mide: el modelo
local no sustituye una medición.**

## 8. Regla de oro, si sólo se recuerda una cosa

> **Pídele que reconozca formas, nunca que entienda comportamientos.**
>
> «¿Qué hay aquí que viole esta regla?» → sí.  
> «¿Por qué esto se comporta así?» → no.

Y verifica siempre. No por desconfianza: porque **verificarlo es barato y equivocarse no**. Sus dos
peores entregas —la fórmula del 006 y el diagnóstico del 009— habrían pasado desapercibidas si
alguien se hubiera fiado de lo bien que suenan.

---

## 9. Qué se deduce de esto para optimizar Programator

Lo que sigue **no es sobre el modelo, es sobre el arnés**: cinco cambios que atacan modos de fallo
observados, ordenados por lo que ahorrarían.

### 9.1. La guía debe nombrar `leer_fichero` (ataca el modo de fallo 3)

**`leer_fichero` existe desde la 0.2.x** (`src/arnes/herramientas.rs`, acotada a la carpeta del
proyecto) y estaba declarada al modelo en `plantillas/preambulo-del-modelo.md`. Lo que faltaba no era
la herramienta, sino que la guía la mencionara:

- **La guía afirmaba lo contrario.** `plantillas/como-encargar-a-programator.md` decía «tienes tú
  herramientas y él no» (`como-encargar-a-programator.md:65`). Por eso quien dirigía pegaba los
  ficheros enteros en el encargo —19 KB en una auditoria—, desbordando la ventana de 16.384 tokens.
- **El encargo 004 no duplicó por ignorancia del arnés.** Duplicó una constante que estaba en otro
  fichero que nunca le vio, porque la guía no le decía que podía leerlo.
- **El propio modelo lo sabía.** En el banco de pruebas, al preguntarle qué herramientas tenía,
  enumeró su repertorio empezando por `leer_fichero`. Quien lo dirigía leía una guía que decía lo
  contrario.

**Lo arregló la 0.10.0**: genera esa sección de la guía desde la ficha del repertorio, así que nunca
vuelve a ocultarle una herramienta que tiene. El cambio con más retorno no era código nuevo: era
**contar la verdad en la guía**.

### 9.2. Versionar los candidatos en vez de sobrescribirlos (encargo 006)

Entregó **tres versiones del mismo fichero en diez minutos** y cada una borró a la anterior sin
aviso, de modo que **la revisada no fue la que quedó en disco**. Y no eran equivalentes: una era
correcta y las otras dos fallaban en la misma línea de formas distintas.

Escribir `nombre-001.cs`, `nombre-002.cs`… o rechazar la segunda entrega del mismo encargo lo
cierra. Hoy la única defensa es que el humano se acuerde de copiar el fichero antes de leerlo.

**Implementado en la 0.10.0.** Ahora cada candidato se versionan y el histórico los retiene.

### 9.3. Avisar del tamaño del canal antes de desbordar la ventana

El 23/09 los ficheros del canal llegaron a **35.000 tokens** frente a la ventana configurada de
**16.384** en el repositorio por defecto (`src/config.rs`, función `contexto_por_defecto`),
y el servidor empezó a rechazar **todas** las peticiones en bucle. **No falla con un aviso claro:
falla por debajo, reintentando**, y desde fuera parece que ha dejado de atender sin más.

Dos cosas lo arreglarían: **un aviso al superar un umbral** (por ejemplo el 70 % de la ventana
configurada) y **un mensaje explícito en el buzón** cuando el motor rechaza por contexto, en vez del
`status code 400` escueto que costó una tarde diagnosticar.

**Implementado en la 0.10.0.** Ahora calcula el umbral sobre la ventana en vigor y explica el
rechazo con palabras.

### 9.4. No re-atender lo que se archiva (medido el 23/09)

Los históricos del canal viven **dentro de** la carpeta que el arnés sondea, así que **archivar un
encargo se lo vuelve a encargar**: el encargo 009 se archivó a las 20:44 y lo entregó a las 20:47,
gastando tres minutos de inferencia en trabajo ya cerrado. Hoy la defensa es degradar el encabezado
a mano al archivar. El arnés podría ignorar ficheros por patrón (`historico-*.md`) o llevar una
lista de encargos ya atendidos por huella.

**Implementado en la 0.10.0.** Descarta automáticamente los históricos por patrón.

### 9.5. Plantillas de encargo con las advertencias ya dentro

**Medido, y es el hallazgo más rentable de los catorce encargos**: nombrarle dentro del encargo el
fallo que cometió en el anterior **se lo desactiva**, pero sólo si es un fallo de *atención*:

| Advertencia puesta en el encargo | Clase | Resultado |
|---|---|---|
| «citas valores en el miembro equivocado» | atención | **14 de 14 ubicaciones correctas** |
| «llamaste hardcodeo a una constante con nombre» | atención | **0 falsos positivos** |
| «comprueba que lo que propones no sea ya lo que hace el código» | comprensión | **lo repitió dos veces** |

Si el arnés ofreciera plantillas de encargo por tipo —auditoría, función acotada— con esas
advertencias ya escritas, el acierto subiría sin depender de que quien dirige se acuerde. Y
conviene que la plantilla de auditoría **le pida explícitamente decir «no encuentro nada»**: en el
encargo 013, ante un fichero limpio, rellenó con cuatro hallazgos inútiles en vez de callar.

### 9.6. Un apunte menor pero barato

El comprobador de propuestas no cubre `.md`: toda auditoría se entrega con «⚠️ SIN COMPROBAR — no hay
comprobador configurado para «.md»». Los comprobadores por extensión **ya son configurables desde la
0.9.0** en `[verificacion.comprobadores]` del TOML. Lo que hace falta es añadir la clave `md` en el
TOML del proyecto, apuntando a un comprobador que valide que el fichero no está vacío y que cada
línea cumple el formato pedido (`TIPO | miembro | qué | qué harías`). No es código nuevo: es
configuración.

---
**Cómo mantener este documento**: al cerrar un encargo en uso real, añade su fila a la tabla del §1
y, si el resultado contradice algo de aquí, corrígelo con la medida delante. La copia maestra es
ésta; el proyecto NatureLand conserva sólo el resumen operativo, en
`.gestor/desempeno-programator.md`, junto a la guía de cómo escribirle un encargo
(`.gestor/canal/COMO-ENCARGAR-A-PROGRAMATOR.md`), que es el único de los dos que lee él.
