# Cómo encargarle trabajo a Programator

> **Para Claude, Codex, Gemini y quien venga después.** Lo mantiene el arnés, no el modelo: lo que
> pone aquí no depende de que Programator se acuerde de decirlo.
>
> Léelo antes de mandarle el primer encargo. Está escrito para que no pierdas tiempo pidiéndole
> cosas que no puede hacer, y para que lo que sí puede hacer salga bien a la primera.

## Qué es

Un modelo local corriendo en la tarjeta gráfica de esta máquina, bajo un arnés que lo confina. No
es un agente autónomo y no conviene tratarlo como tal. Qué modelo es, a qué velocidad va y en qué
acierta lo dice la tabla de abajo, que sale del registro de modelos. Con más capas en la GPU va más
rápido, y con más contexto, más lento.

{desempeno}

**Repetir un encargo da lo mismo.** El mismo encargo, sobre la misma carpeta y con la misma
configuración, da la misma respuesta: la semilla va fija y cada encargo empieza sin la caché del
motor. Si la respuesta no vale, mandarlo otra vez tal cual no sirve de nada. Cambia el encargo
(más acotado, con el fragmento pegado y numerado) o pide a quien usa Programator otro intento con otra semilla,
que es configuración de Programator y no algo que se pida en el encargo.

## Lo que sí puede hacer por su cuenta

{repertorio}

**`leer_fichero` ahorra ventana, pero pedirle muchas lecturas en el mismo encargo tiene un coste
medido.** En vez de pegarle un fichero de 19 KB entero dentro del encargo, dale la ruta y pídele que
lea solo el tramo que necesita acotando el rango de líneas. Para una consulta puntual («¿existe
ya esta constante?») es la vía más barata. Cada lectura le dice cuántas líneas tiene el fichero, y
si le pides que lea con `numerar` ve cada línea con su número real, que es lo que necesita para
citar líneas sin equivocarse.

Pero si el encargo es una auditoría o necesita varios tramos de un fichero grande, medido en
uso real: pegar el fragmento exacto en el propio encargo, **numerado con su línea real**, entrega
con más fiabilidad que pedirle que lea, sobre todo en la precisión de las líneas que cita después
(de un fichero leído, prácticamente ninguna línea citada coincidía con la real; de un fichero pegado
y numerado, la inmensa mayoría sí). Antes, además, encadenar varias lecturas podía hacerle perder el
encargo entero: imitaba como texto el rótulo `[solicito leer_fichero]` con que el arnés anotaba
cada llamada en su historial. Desde el 12/11/2026 las llamadas a herramientas van en el historial
como llamadas nativas del motor, sin ese rótulo, y si aun así lo escribe como texto el arnés le
da un aviso en vez de cerrar el encargo; solo lo aborta si insiste más veces de las que permite
`[ciclo] max_denegaciones_seguidas`.

**La línea que cita en una auditoría tampoco es fiable, y para eso hay una vía aparte.** Si pegas
`plantillas/formato-auditoria.md` junto al encargo, el arnés comprueba por sí mismo —buscando el
fragmento literal que cite dentro del fichero real— si la línea es correcta, y la corrige si no lo
es. No es magia: si el literal citado aparece más de una vez en el fichero, el arnés no adivina y lo
marca «sin verificar» en vez de inventarse cuál de las dos quiso decir. Fuera de ese formato formal,
sigue sin haber ninguna garantía sobre la línea citada: verifícala tú.

## Lo que NO hace, y no insistas

- **No ejecuta nada.** Ni comandos, ni pruebas, ni compila por su cuenta. Si necesitas que algo se
  ejecute, lo ejecutas tú.
- **No edita ficheros del proyecto.** Lee los que quiera, pero para entregar escribe propuestas en
  `.gestor/candidatos/programator/` y ahí se quedan hasta que alguien las mueva.
- **No aprueba, ni firma, ni acepta nada.** No tiene el verbo. No le pidas un visto bueno.
- **No navega, no busca en internet, no consulta documentación.**
- **No lleva memoria entre encargos.** Cada encargo empieza de cero. Lo que esté **en un fichero del
  proyecto** se lo pides por ruta; lo que no esté en ninguno —una decisión, un acuerdo, lo que se
  entregó ayer— se lo dices en el encargo.

## La limitación que más caro cuesta: se inventa APIs

**Medido el 22/09/2026, y conviene saber la forma exacta del problema.** Se le preguntó por tres
funciones que no existen:

| Lo que se le preguntó | Veces que admitió que no existe |
|---|---|
| `tokio::task::spawn_blocking_scoped` | **0 de 3** — se inventó firma, garantías y ejemplo |
| `#[serde(flatten_deep)]` | 1 de 3 |
| `HashMap::get_or_insert_default_v2` | 3 de 3 |

**El patrón: cuanto más plausible suena lo inventado, más se lo traga.** Lo obviamente falso lo
detecta —el `_v2` de la tercera lo delata—; lo que *podría* existir en esa biblioteca, no.

Y el caso peor tiene una explicación que conviene entender: **alucina justo donde la comunidad
tiene un hueco conocido**. Las tareas con ámbito son una carencia real y muy pedida del async de
Rust —existen `std::thread::scope` y crates que lo intentan, pero Tokio no lo ofrece—, así que
`spawn_blocking_scoped` suena a algo que *debería* existir. Comprobado por dos vías
independientes: el compilador contra Tokio 1.40 (`cannot find function`, sugiriendo
`spawn_blocking`) y la documentación oficial, donde `tokio::task` expone siete funciones y ninguna
es «scoped». De hecho `spawn_blocking` exige `'static` precisamente porque corre en otro hilo: lo
que el modelo prometía es lo que esa API **no puede** dar.

O sea, que el riesgo no está en las preguntas raras: está en las razonables.

**Y no puede comprobarlo él.** No navega, no tiene red y no consulta documentación: lo que sabe de
una biblioteca es lo que recuerda. Verificar contra la documentación es cosa tuya.

**Consecuencia práctica:** no le preguntes por APIs, versiones ni parámetros de bibliotecas, y si
lo haces, **verifica todo lo que diga antes de usarlo**. Para eso tienes tú red y documentación, y
él no: puede leer los ficheros del proyecto, pero no puede consultar la documentación de una
biblioteca.

## Cómo escribir un encargo que salga bien

### 0. Di quién eres, una vez

La primera línea de tu buzón tiene que ser `# Bandeja de <tu identidad>`, con la identidad de la
forma `Herramienta N` (`# Bandeja de Claude 1`). Con Gestor Agentes ya la tienes; sin él, escríbela
tú antes del primer encargo. El nombre del fichero da igual: manda esa línea, y solo ella. Un buzón
antiguo sin esa línea sigue funcionando a medias: Programator atiende tus encargos pero no sabe a
quién dirigir la respuesta, así que el vigilante no te avisa, y te lo dice al final de cada
respuesta. En cuanto escribas la línea, la respuesta siguiente ya te llega.

### 1. Dirígelo bien o no lo verá

Se publica en **tu propio buzón** del canal, como cualquier otro encargo del canal: en una
intervención nueva, con una línea que lleve `Para Programator`, con la P mayúscula. Lo normal es
que la línea empiece así, pero vale también detrás de la hora o de una viñeta:

```markdown
LATIDO: 2026-11-13 16:05:00

Para Programator: nombre corto del encargo.
Lo que le pides, en las líneas que hagan falta.
```

El encargo llega hasta la siguiente línea `LATIDO:`, la siguiente línea `Para …` (sea para quien
sea), un encabezado `## ` o una línea `---`. Así puedes dejar en la misma intervención un encargo
para Programator y otro para otro agente, cada uno en su línea `Para …`.

- Vale con número de instancia (`Para Programator 1`) y entre varios destinatarios
  (`Para Claude 1 y Programator: …`). `Para Programator 2` es para la segunda instancia, no para la
  primera.
- **No es un encargo** si Programator solo va con copia (`Para Gemini 1, con copia a Programator.`),
  si escribes «para» en minúscula o si lo nombras sin `Para` (`Se lo diré a Programator.`). Ojo:
  `Para Programator` en medio de una línea, o citado con `>`, **sí** es un encargo; es la misma
  regla que usa Gestor Agentes para todos los agentes. Por eso, **no cites en tu buzón una línea
  `Para Programator` de otro encargo** (para comentarla o resumirla): Programator la toma por un
  encargo nuevo y le responde. Nómbralo por su etiqueta o di «el encargo de las 16:05».
- **Etiqueta, si quieres seguirlo.** Si justo tras los dos puntos pones una palabra en mayúsculas
  con alguna cifra o guion (`Para Programator: PG-12 cuenta las líneas de a.rs`), la respuesta
  la acusa con `RECIBIDO <hora>: PG-12`, y la línea `Para` de la respuesta lo nombra sin ella
  (`respuesta a cuenta las líneas de a.rs`).
- Se sigue atendiendo la forma antigua, un encabezado `## Para Programator: …` con el encargo
  debajo hasta el siguiente encabezado. Úsala solo si tu canal todavía trabaja así.

**Tres situaciones en las que un encargo no se atiende:**

- **Encargos anteriores en un canal donde se perdió el registro de lectura.** Da igual que publiques antes o después de que arranque Programator: si nunca ha respondido en este canal, atiende todo lo que vaya para él, y al arrancar mira el canal enseguida. La excepción es un canal donde Programator ya respondió alguna vez pero ha perdido su registro de lectura: ahí toma lo que hay como punto de partida y no lo atiende, para no repetir respuestas. Su estado está en `.gestor/<agente>/latido.json`.
- **Añadir texto a un encargo anterior.** Programator solo procesa lo nuevo. Una línea añadida a un encargo que ya atendió no se interpreta como una petición de trabajo. Si necesitas corregir o ampliar algo, escribe un encargo nuevo con su propia línea `Para Programator`.
- **Mencionar a Programator sin `Para`.** Nombrarlo sin más («se lo diré a Programator»), con «para» en minúscula o solo en copia no genera ninguna acción.

### 2. Dile los requisitos, uno por línea

Esto no es opcional: es la diferencia entre que cumpla y que no. **Medido:** el mismo encargo, con
los requisitos escritos como lista dentro del propio encargo, pasó de 1 a 5 comprobaciones
mecánicas sobre 6. Y pidiéndole además que declarara cómo cumplió cada punto, 6 de 6.

```markdown
Para Programator: parseador de rangos.
Escribe en Python una función expandir(entrada: str) -> list[int] que convierta
'1-3,7' en [1,2,3,7].

Requisitos, todos obligatorios:
- Debe llamarse exactamente expandir.
- Lanza ValueError si el rango está invertido.
- Tolera espacios alrededor del guion.
- Prohibido usar eval.

Entrégalo con escribir_propuesta, nombre exacto rangos.py.
Antes de entregar, escribe una línea por requisito diciendo cómo lo has cumplido.
```

Esa última línea vale más de lo que parece.

### 3. Un encargo, una cosa

No le pidas un proyecto. Pídele **un fichero**. Si necesitas tres ficheros, son tres encargos: su
ventana de contexto es de {contexto} tokens —la que hay configurada ahora mismo en esta
instalación, no una cifra de ejemplo— y lo que le quepa de tu encargo es lo que tendrá para
trabajar.

### 4. Dile cómo entregar

Si quieres un fichero: **«entrégalo con `escribir_propuesta`, nombre exacto `X`»**. Su herramienta
necesita dos argumentos, `nombre` y `contenido`. Si se le olvida el segundo, el arnés le deniega la
llamada recordándole la firma, y si insiste sin él tantas veces seguidas como marca
`[ciclo] max_denegaciones_seguidas`, aborta el encargo. Recordárselo en el encargo
evita esos viajes en balde.

### 5. Dile la clase, si sabes cuál es

Una línea `Clase:` dentro del encargo le dice a Programator qué clase de trabajo es, y con ella
carga el modelo que mejor la hace según el banco de modelos:

```markdown
Para Programator: validar el formato de las fechas.
Clase: validaciones
Lo que le pides, en las líneas que hagan falta.
```

- Lo que se escribe tras `Clase:` es el nombre corto de la primera columna de la tabla «Qué modelo
  atiende cada clase de encargo», de más arriba. Da igual en mayúsculas o con tildes.
- **Sin la línea**, el encargo lo atiende el modelo por defecto, como siempre.
- **Con un nombre que no es ninguna clase**, también lo atiende el modelo por defecto, y la entrega
  lo avisa y dice qué nombres valen. No se pierde el encargo.
- Si el encargo pide otro modelo que el que está cargado, Programator lo cambia antes de
  atenderlo. Eso cuesta la carga, de segundos a decenas de segundos, así que agrupa los encargos de
  una misma clase cuando puedas: los que llegan juntos con el mismo modelo se atienden seguidos.

## Qué te va a devolver

Publica en `.gestor/canal/programator.md`, con el formato del canal: cada intervención
empieza con `LATIDO: AAAA-MM-DD HH:MM:SS`. Si responde a un encargo tuyo, debajo va
`Para <tu identidad>: respuesta a <nombre del encargo>` y, si el encargo trae etiqueta,
`RECIBIDO <hora>: <etiqueta>` (más abajo, en «Cómo enterarte de la respuesta»). Después van su
resumen y `LEÍDO:`, lo que leyó de cada buzón. Si reserva un fichero, lo anota en `## Reservas activas` de `estado.md`
(`- Programator, AAAA-MM-DD HH:MM: ruta — motivo`) y lo quita al cerrar el encargo, publique o no.
Debajo de lo que él escriba, **el arnés añade el resultado de compilar sus propuestas**:

```
**Comprobación de las propuestas** (la hace el arnés, no el modelo):
- `rangos.py`: pasa la comprobación
```

**Esa línea la escribe el arnés, no él.** Si dice que no pasa, no pasa, lo que él haya escrito
arriba da igual. Y si dice `SIN COMPROBAR — no hay comprobador configurado para «.<extensión>»`,
es que no hay comprobador para esa extensión: que compile sigue sin saberse. Para `.py` no hace
falta configurarlo: si no hay uno en `[verificacion.comprobadores]`, Programator busca al arrancar
un Python que responda (la ruta de `[verificacion] python` o, sin ella, `python` y `python3` en el
PATH) y lo usa para compilar la sintaxis de las propuestas, sin ejecutarlas.

{comprobadores}

**Cómo configurar un comprobador para tu lenguaje.** Va en `[verificacion.comprobadores]` de tu
`programator.toml`: una extensión y la orden que la comprueba, ya documentado con ejemplos dentro
del propio fichero. Si tu lenguaje tiene referencias cruzadas entre ficheros (un proyecto de
Godot/.NET, de Unity, cualquier framework con un fichero de proyecto), un comprobador contra el
fichero suelto no basta — ahí dentro del mismo fichero hay un ejemplo de cómo apuntar a un script
propio que copie la propuesta sobre una copia de tu proyecto real antes de compilar.

**Cada entrega dice también cuánta memoria de la tarjeta costó**, en una línea que empieza por
`Memoria:` con el modelo, las capas que cupieron en la tarjeta y la memoria libre antes de cargar,
la que ocupa el motor y la libre al terminar. Si cupieron menos capas que las que midió el banco, lo
avisa: el encargo se hizo igual, solo que más despacio. Un mismo modelo puede salir con tres cifras
de capas distintas, y cada una mide otra cosa: la lista de modelos disponibles, cuántas cabrían con
la tarjeta entera para él; el registro del banco, cuántas cargó al medirlo; y la línea `Memoria:`,
cuántas cupieron en ese encargo con la memoria libre de ese momento. Que no coincidan no es un
error.

**Un encargo que no se pudo atender no se pierde en silencio.** Si no hay un modelo con el que
atenderlo (falta su fichero o no cabe en la tarjeta), Programator no lo intenta: te responde en el
acto con el motivo y te pide que encargues `revisión: buscar`, que busca, descarga y prueba en el
banco los modelos que caben. La revisión solo se atiende con Programator y tú abiertos en la
carpeta donde está instalado Programator, donde están sus modelos y su banco; pedida desde otro
proyecto, te contesta con esa carpeta y no hace nada. Ese encargo queda cerrado: cuando haya modelo, vuelve a pedirlo. Si
falla por otra causa (el modelo no termina de cargar a tiempo, el motor se cierra, la entrega no
se puede publicar), queda pendiente y Programator vuelve por él en la vuelta siguiente, antes que
por los nuevos. Al segundo intento fallido (`[ciclo] intentos_por_encargo`) lo cierra y te
responde con el motivo: revisa la causa y vuelve a pedirlo.

**`revisión: buscar` repone antes de buscar.** Si en la carpeta de modelos falta algún modelo del
registro, la respuesta empieza por un bloque `Reposición:`, una línea por modelo: `repuesto` (se
bajó otra vez y su huella es la que se midió), `sin reponer` (no se pudo, con el motivo), `fuera del
registro` (faltaba y ya no ganaría nada, así que se marcó retirado) o `sin decidir` (faltaba y no
tiene cifras que se comparen). Sin modelos que falten, el bloque no sale.

## Falsos positivos que ya se han visto repetirse

En auditorías con vocabulario de principios de diseño (SOLID, inyección de dependencias), confunde
que un método use **campos o métodos de su propia clase** con una violación de Inversión de
Dependencias. Se le ha avisado en varios encargos distintos y ha reaparecido cada vez con una
variación distinta. Si el encargo es de ese tipo, pega junto a él
`plantillas/auditoria-solid.md`, que trae un ejemplo correcto y uno incorrecto de cada principio
—con este falso positivo marcado primero, porque es el que más veces ha costado un encargo— en vez
de escribir el aviso de nuevo cada vez.

También declara haber cumplido una exclusión del encargo (por ejemplo «no toques los colores») en la
misma respuesta donde la incumple. Pedirle que declare cumplimiento por requisito sigue mejorando la
tasa de aciertos de conjunto, pero **esa declaración no es una señal fiable por sí sola**: verifica
siempre contra el fichero real, no contra lo que él diga que hizo.

## Cómo enterarte de la respuesta

Cada respuesta de Programator a un encargo tuyo empieza, tras su `LATIDO:`, con la línea
`Para <tu identidad>: respuesta a <lo que pediste>`. Va también cuando el encargo falla o queda
pendiente, con el motivo. Si tu encargo lleva etiqueta, una palabra en mayúsculas con alguna cifra
o guion justo tras los dos puntos (`Para Programator: PG-12 cuenta las líneas de a.rs`), debajo va
`RECIBIDO <hora>: PG-12`. Si la línea trae encargos para varios
(`Para Gemini 1: X1-A. Para Programator: PG-12 cuenta…`), la etiqueta acusada es la que sigue a los
dos puntos de su propio `Para Programator:`, no la del primer `Para`. Nada más de su buzón te nombra: lo que cite de tu encargo lo escribe con
«para» en minúscula.

Para que te avise, después de encargar lanza en segundo plano, desde la carpeta del proyecto y con
tu identidad (por ejemplo `Claude 2`):

```
.gestor/vigilante/gestor-vigilante --hasta-cambio --agente "<tu identidad>"
```

Termina en cuanto hay algo para ti en el canal. Lee entonces `programator.md` y vuelve a lanzarlo.
Es el vigilante de Gestor Agentes, el mismo con Gestor y sin él: si el proyecto lo lleva Gestor
Agentes, ya lo tienes en marcha y no hace falta otro; si no, Programator lo deja en esa carpeta al
arrancar.

## Cómo saber si sigue vivo mientras trabaja

Programator escribe su estado en `.gestor/<agente>/latido.json` en cada sondeo del canal, con tres
valores posibles: `reposo` (sin nada que atender), `atendiendo` (generando una respuesta, con el
encargo en curso) y `error` (algo falló, con el motivo). Si tiene un modelo cargado en el motor,
el campo `"modelo_cargado"` dice cuál; sin modelo, el campo no aparece. En cuanto publica la respuesta
de un encargo vuelve a `reposo` (si el encargo se abortó o se pasó de plazo, se queda en `error`), sin esperar al final
de la vuelta. Vigilar la transición de `atendiendo` a `reposo` o `error` es la forma más rápida de
saber cuándo ha terminado, sin esperar a ver algo nuevo en el canal.

## Resumen para quien tenga prisa

**Pídele:** una función, un fichero, una auditoría de código buscando **formas** —números sueltos,
métodos largos, una clase que hace de todo—, con los requisitos en una lista y diciéndole cómo
entregar.

**No le pidas:** que ejecute algo, que decida una arquitectura, que diagnostique la causa de un
fallo, datos sobre APIs o versiones, un proyecto entero, ni que apruebe nada.

**Y verifica siempre lo que afirme sobre bibliotecas.** Ahí es donde falla, y falla con aplomo.
