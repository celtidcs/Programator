# Hoja de ruta de Programator

Qué falta, en qué orden y por qué ese orden. Cada versión se hace **paso a paso**: primero su
especificación, luego su plan, y el plan se ejecuta tarea a tarea con revisión entre cada una.

Estado al escribir esto: la versión 0.10.3 está integrada en `main` desde el 30/09/2026, con 471 pruebas pasando limpiamente en Windows y en Linux. El ciclo completo se ejecuta contra el modelo local a 18,4 tokens por segundo, y ya se ha empleado durante una jornada completa en un proyecto real (NatureLand, Godot/.NET/C#, 23/09/2026), logrando cuatro entregas aceptadas tras verificación independiente. La 0.10.0 resolvió las carencias del repertorio y observabilidad, la 0.10.1 abrió la compatibilidad con Linux, la 0.10.2 añadió lectura acotada por líneas y aviso explícito ante ciclo sin entrega, y la 0.10.3 humaniza los mensajes de inicio en consola y ofrece la orientación directa para colaborar con el modelo de pago.

---

## El criterio del orden

Tres reglas ordenan lo que queda, y conviene decirlas porque explican por qué lo más vistoso no va
primero:

1. **Lo que hace visible lo que ya existe, antes que lo nuevo.** Fue el criterio que puso la 0.5.0
   por delante de todo lo demás: la 0.4.0 calculaba el encaje y ese cálculo estaba enterrado detrás
   de un encargo, así que podía pasar días sin aparecer. Un trabajo que no se ve no se puede juzgar,
   y lo que no se puede juzgar no se puede corregir.
2. **Lo que quita trabajo manual, antes que lo que añade capacidades.** Montar la carpeta portable a
   mano se hace una vez y se olvida; mantenerla al día se hace siempre.
3. **Nada que dependa del ciclo completo hasta que el ciclo completo funcione contra el modelo
   real.** Esa prueba sigue sin ejecutarse.

---

## Lo publicado el 22/09/2026, en un día

El ciclo completo se ejecutó por primera vez contra el modelo real, y de ahí salieron cuatro
versiones más en cadena:

| | Qué trajo |
|---|---|
| 0.6.0 | **El motor que se afina**: `--parallel` explícito y el encaje contando la caché por secuencia. De 9,8 a 18,4 tokens por segundo |
| 0.7.0 | **La puerta de verificación**: el arnés compila lo que el modelo entrega y publica el veredicto |
| 0.7.1 | **El primer encargo deja de colgarse** cuando el motor está cargando |
| 0.8.0 | **El sitio importa**: las normas de código pegadas al encargo, no al sistema |

Lo que queda por debajo conserva su orden; solo han corrido de número.

---

## 0.5.0 — El arranque que se explica (publicada el 17/09/2026)

**Qué resuelve.** Hasta la 0.4.0, Programator arrancaba, decía en qué carpeta trabajaba y se
callaba. No decía qué GPU había visto, ni si el modelo existía, ni cuántas capas pensaba cargar, ni
qué se le podía pedir. Todo eso lo sabía, o lo podía saber en milisegundos, pero estaba enterrado
detrás de un encargo.

- **El aviso de arranque**: GPU y VRAM libre, modelo y su peso, encaje decidido. Siempre, antes de
  entrar en el bucle, sin esperar a que haya un encargo.
- **`--ayuda`**, con los comandos disponibles, y una línea en el arranque que diga cómo verla.
- **Verificación de las piezas del motor**: que `llama-server.exe` y sus bibliotecas estén donde
  deben, con un aviso que diga cuál falta, en vez de fallar al lanzar el proceso.

**Por qué iba primera.** Es la que convierte un programa mudo en uno que se explica, y era barata: el
cálculo ya existía y solo había que invocarlo antes.

**Publicada.** Trajo también `--version` y `--diagnostico`, y el aviso de un motor sin CUDA —el caso
real que lo motivó: el `llama-server` que instala Docker trae veinticuatro bibliotecas y todas son
de CPU—. Queda pendiente su única verificación
con la GPU real encendida, descrita como Prueba D en `docs/pruebas-manuales.md`.

## 0.9.0 — Programator en cualquier proyecto (publicada el 23/09/2026)

**Qué resolvió.** Hasta aquí, Programator daba por supuesto que el proyecto anfitrión era MMCelt:
copiaba la carta fundacional de ese equipo entera —30.711 bytes con siete `cargo` y cero Godot— en
la raíz de cualquier repositorio, y no arrancaba sin una persona delante eligiendo carpeta en un
diálogo.

- **La plantilla neutra**, de 30.711 bytes a 2.684, con una prueba barrera que impide la recaída.
- **La sección de verificación la genera el arnés** desde los comprobadores del TOML, que es lo que
  de verdad ejecuta. Lo que el modelo lee y lo que el arnés hace ya no pueden divergir.
- **`--ruta` y memoria de la última carpeta**: se reinicia sin nadie delante.
- **`PROGRAMATOR.md` deja la raíz ajena** y baja a `.gestor/<agente>/`, sin romper las instalaciones
  que ya lo tenían arriba.
- **«SIN COMPROBAR» en vez de «sin verificar»**, y la guía publica la ventana de contexto en vigor.

**Por qué adelantó al motor bajo control.** Por el criterio 2 de esta hoja: lo que quita trabajo
manual va antes que lo que añade capacidades. Montar el motor a mano se hace una vez; adaptar la
plantilla y elegir la carpeta a mano se hacía en cada proyecto y en cada reinicio.

## 0.10.0 — El arnés cuenta la verdad (integrada el 26/09/2026)

**Qué resolvió.** La jornada en NatureLand demostró que los problemas de información del arnés inducían a error a quien lo dirigía. Esta versión unifica el repertorio de herramientas y dota al sistema de observabilidad completa y recuperación ante fallos.

### Lo que trajo

- La ficha del repertorio (`src/arnes/ficha.rs`): fuente única de verdad para los ocho verbos del arnés, que genera automáticamente el preámbulo del modelo y la guía de los agentes, e informa al motor de inferencia sobre los nombres y tipos de los argumentos.
- Denegación con la firma completa de la herramienta para que el modelo pueda corregir sus llamadas sin tener que adivinarlas.
- Barreras de redacción que impiden la reaparición de descripciones de herramientas escritas a mano dentro de las plantillas.
- Versionado correlativo de las entregas de candidatos para no sobrescribir trabajo previo.
- Exclusión automática de los históricos del canal por prefijo.
- Control de la ventana de contexto mediante avisos preventivos y explicación comprensible de las causas de rechazo del motor.
- Archivo de latido en tiempo real (`.gestor/<agente>/latido.json`), escrito en cada sondeo para informar si el agente está en reposo, atendiendo o en error, y cuándo ocurrirá la siguiente comprobación.
- Aborto honesto que comprueba el disco e informa con precisión sobre qué propuestas se han guardado.
- Detección y colapso de llamadas repetidas idénticas para evitar el consumo innecesario del cupo de herramientas.
- Publicación inmediata de entregas intermedias en el buzón del canal.
- Reintento automático ante contención transitoria de la tarjeta gráfica tras verificar la salud del servidor.
- Mensajes de error en la terminal estructurados con el fallo concreto, su consecuencia operativa y la acción recomendada.
- Avisos de disponibilidad en consola que informan con claridad del inicio de la escucha y de la cadencia de sondeo tanto en el primer arranque como tras un reinicio.
- Parámetros avanzados en el archivo de configuración para estimación de tokens, umbral de advertencia, prefijos ignorados, registro de desempeño y reintentos del motor.
- Nuclearización del código fuente en módulos cohesivos (`herramientas/`, `sesion/` y `arranque/`).
- Ampliación de la suite de pruebas automatizadas hasta 435 comprobaciones mecánicas independientes.

## 0.10.1 — Programator también en Linux (publicada el 27/09/2026)

**Qué resolvió.** Que no hubiera versión de Linux no era una decisión de diseño, sino trabajo sin hacer: lo único atado a Windows era medir la tarjeta gráfica y unos cuantos textos de la terminal. La medición en Linux se hace ahora preguntando al programa que acompaña al controlador de NVIDIA, y los textos que dependen del sistema viven en un módulo propio.

De paso se resolvió otra cosa que estorbaba a cualquiera que llegara nuevo: las dos piezas pesadas, motor y modelo, había que ir a buscarlas a mano. Ahora las trae un script, con las versiones fijadas y verificando cada descarga, y un documento explica qué hacer cuando alguna de esas direcciones deje de existir.

**Lo que queda pendiente de ahí.** Nadie ha arrancado todavía Programator en un Linux con una tarjeta gráfica delante, así que la medición real y la carga del modelo siguen sin verificar en ese sistema.

## 0.10.2 — Lectura acotada y aviso de entrega (publicada el 30/09/2026)

**Qué resolvió.** Experiencias reales en NatureLand mostraron dos limitaciones prácticas del arnés: el desbordamiento de ventana por lecturas masivas y la falta de señal cuando un ciclo terminaba sin invocar herramientas de entrega.

- **Lectura acotada por líneas**: `leer_fichero` soporta ahora `desde_linea` y `hasta_linea` (base 1), permitiendo inspeccionar secciones específicas de ficheros grandes sin saturar la ventana ni desbordar el tope de bytes.
- **Tipado en esquema de herramientas**: los argumentos declaran su tipo JSON (`integer`, `string`) hacia el servidor de inferencia.
- **Aviso explícito ante ciclo sin entrega**: publicación visible en el canal y alerta en terminal ante `Desenlace::SinEntrega`.
- **Higiene del repositorio**: purga de ramas fusionadas y worktrees obsoletos.

---

## 0.11.0 — El motor bajo control

**Qué resuelve.** Hoy la carpeta portable se monta a mano y nadie sabe si el motor se ha quedado
atrás. La 0.10.0 arregló cómo cuenta el arnés lo que hace; la 0.11.0 arregla cómo maneja el motor
que subyace.

- **La versión del motor se fija en el TOML** (por ejemplo `b10993`) y el arranque comprueba si hay
  publicaciones más recientes, **sin descargar nada**.
- **`--actualizar-motor`**: descarga la versión indicada, verifica su integridad y la instala.
- **`--descargar-modelo`**: lo mismo para un modelo declarado en la configuración.

**La decisión que manda aquí**, y conviene tenerla escrita: llama.cpp publica **varias compilaciones
al día**, sin probar, de 254 MB cada una más 391 MB de runtime de CUDA. Un agente desatendido que se
actualice solo arrancaría cada noche con un motor distinto que nadie ha probado. Por eso el motor
**solo cambia cuando el Director lo manda**, y el arranque se limita a avisar de que hay algo más
nuevo.

## 0.12.0 — Repartir la memoria por tensores

**Qué resuelve.** Hoy `linea_de_arranque` compone siete parámetros fijos en el código, y el encaje
decide *cuántas capas* caben. Para modelos más grandes que la tarjeta, la pregunta útil no es cuántas
capas, sino **qué va a dónde**.

- **Exponer los parámetros de `llama-server`** en el TOML, en vez de tenerlos incrustados.
- **`--override-tensor` y `--n-cpu-moe`**: mandar tensores concretos a la CPU por patrón. Es la
  técnica que hoy permite correr modelos de expertos enormes dejando la atención en GPU y los
  expertos en RAM.
- **`--no-mmap` y `--mlock`**: decidir si el modelo se mapea desde disco o se fija en RAM.
- **El caché KV en CPU**, y elegir su cuantización por separado para K y para V.

**Lo que cambia respecto a la 0.4.0.** El encaje pasa de decidir un número a decidir un reparto. La
aritmética que ya existe sigue valiendo; lo que crece es la política.

## Después, sin comprometer orden

- **Base de conocimiento (RAG)** sobre la documentación de `N:\libros-programacion-gratis-main`: 59
  PDF y 58 EPUB, 291 MB. Exige trocear, calcular incrustaciones, guardar un índice y añadir un verbo
  nuevo al repertorio del modelo. **No tiene dónde enchufarse hasta que el ciclo completo funcione**,
  y por eso no lleva número todavía.

  **Encargo cerrado el 17/09/2026: la biblioteca no aporta nada.** El Director encargó responder si
  esos libros servían de algo antes de construir nada, y la revisión ya se hizo: **su interés para
  los modelos es nulo**. Confirmado por el Director el 17/09/2026.

  Es lo que la sospecha anticipaba. Los 59 títulos son manuales **introductorios y generalistas en
  español** —`java-basico-aprendices`, `python-para-todos`, `cpp-fundamentos-basicos`, `git-pro`,
  `docker-introduccion`, `scrum-desde-las-trincheras`—, sin una sola API propietaria ni nada
  posterior al corte de entrenamiento del modelo. Un RAG aporta donde el modelo **no puede** saber,
  no donde hay millones de páginas públicas, y de esas páginas los libros no son más que un resumen.

  **La consecuencia, que es lo que hay que retener.** Los 291 MB se archivan y no se indexan. Si
  algún día se construye el RAG, **indexará el material propio del proyecto** —la especificación del
  encaje, las normas de la casa, el protocolo del canal, el historial de decisiones—, que es
  exactamente lo que el modelo no puede saber por su cuenta. **No hace falta volver a plantear la
  biblioteca.**

  El contraste de las veinte preguntas que se describía aquí como método de verificación queda **sin
  ejecutar, y sin falta**: era el camino hacia esta respuesta, no la respuesta. Sigue en el historial
  de git por si alguna vez hay que juzgar otra biblioteca.
- **Decisión del motor, cerrada el 17/09/2026.** Se adopta la opción A: `llama-server` como proceso
  aparte con cliente HTTP en `127.0.0.1`, y se descartan los bindings en proceso. Los tres puntos
  correctivos que condicionaban la recomendación están implementados y comprobados. Ver «La
  decisión» en `docs/decisiones/2026-09-14-motor-local-servidor-http-vs-bindings.md`.
- **El reparto entre varias GPU** (`--split-mode`, `--tensor-split`), fuera de alcance mientras haya
  una sola tarjeta.
- **Internacionalización completa (i18n)**: queda pendiente de decisión expresa del Director. Traducir exclusivamente los mensajes de la terminal resultaría engañoso, dejando una consola en otro idioma mientras el grueso del producto continúa en español. La internacionalización íntegra debe alcanzar a las plantillas (preámbulo del modelo, guía para agentes, manual de usuario) y al propio protocolo del canal (detección de encargos con encabezados en español como `## Para <agente>`), lo que constituye una decisión de producto con entidad de versión propia. En la 0.10.0 se consolida la regla de que todo texto visible se compone en funciones puras con nombre, preparando el terreno sin añadir catálogos ni complejidad prematura.


---

## Lo que ya se probó, y lo que sigue sin probarse

**El ciclo completo se ejecutó contra el modelo real el 22/09/2026**, por primera vez desde que
existe el proyecto. Aparecieron dos fallos bloqueantes que ninguna de las 255 pruebas podía cazar
—vivían en la junta entre el arnés y el motor— y los dos están arreglados. El cimiento ya no está
verificado solo en frío.

Lo que sigue sin poder automatizarse son las pruebas manuales, porque necesitan la tarjeta
encendida. Las dos que más valen, y que ya se pasaron una vez:

- **Prueba F**: que el motor arranque con `n_slots = 1` y la generación vaya a 18 tokens por
  segundo o más.
- **Prueba G**: que el primer encargo de la sesión se atienda solo, sin volver a tocar el canal.

Conviene repetirlas tras cada versión que toque el arranque o el motor: son baratas y son las
únicas que miran lo que de verdad pasa cuando el modelo está cargado.
