# Cómo habla Programator con el modelo local: servidor HTTP frente a bindings

**Fecha:** 14/09/2026 · **Estado:** **DECIDIDO el 17/09/2026 — se adopta la opción A.** Ver «La decisión», al final.
**Motivo:** el Director cuestiona el uso de un cliente HTTP y pide máximo control sobre el modelo,
y poder elegirlo, cambiarlo y actualizarlo.

---

## Lo que está en juego

Programator necesita que un modelo local (Devstral Small 2 24B) genere respuestas y **llamadas a
herramientas estructuradas**. Esto último no es un detalle: todo el confinamiento de la Tarea 11
—el repertorio cerrado que concede o deniega— depende de recibir una solicitud con nombre y
argumentos. Si el motor devolviera prosa que hay que interpretar, el arnés dejaría de ser
determinista y pasaría a adivinar.

Hay tres formas de conseguirlo.

| | **A. `llama-server` + HTTP local** | **B. Bindings de `llama.cpp` en el ejecutable** | **C. `llama-cli` por tubería** |
|---|---|---|---|
| Qué es | Un proceso local con el modelo en VRAM; se le habla por `127.0.0.1` | `llama.cpp` compilado dentro del binario de Programator | Lanzar el ejecutable de consola y hablarle por stdin/stdout |
| Estado | **Lo que hay hoy** (Tarea 13) | Requeriría rehacer las Tareas 13 y 14 | — |

**C se descarta de entrada**, y conviene decir por qué para no volver sobre ello: o recarga un modelo
de 24B en cada encargo —decenas de segundos por ciclo, con el disco y la VRAM pagándolo cada vez— o
se negocia en modo interactivo con texto libre. Lo segundo destruye el tool-calling estructurado, que
es el cimiento del confinamiento. No compensa por ahorrarse un puerto local.

La comparación real es **A contra B**.

---

## 1. Control sobre el modelo

Este es el criterio que el Director puso primero, y merece precisión porque es donde más fácil es
equivocarse por intuición.

**A (`llama-server`)** expone por JSON, en cada petición, prácticamente todo el muestreo de
`llama.cpp`: `temperature`, `top_k`, `top_p`, `min_p`, `typical_p`, `dynatemp_range`,
`repeat_penalty`, `presence_penalty`, `frequency_penalty`, la familia `dry_*`, la familia `xtc_*`,
`mirostat` con su `tau` y su `eta`, `seed`, `logit_bias`, `n_probs`, `min_keep` — y, lo más
importante para este proyecto, **`grammar` (GBNF) y `json_schema`**, que permiten *forzar* que la
salida del modelo se ajuste a una forma concreta. La plantilla de chat se controla con
`--chat-template-file`.

Yo suponía que el endpoint compatible con OpenAI sería un techo de control frente al endpoint nativo.
**Me equivocaba, y conviene que conste:** la documentación del servidor dice que
`/v1/chat/completions` acepta también los parámetros propios de `llama.cpp`, no solo el subconjunto
de OpenAI. No hay que cambiar de endpoint para tener control.

**B (bindings)** da más control en términos absolutos: acceso a los logits crudos, posibilidad de
escribir tu propio muestreador, manejo manual del KV cache. Pero ese control viene con una factura
que hay que nombrar: **el binding te da tokens, no llamadas a herramientas**. Tendrías que
implementar tú la plantilla de chat del modelo y el parseo de las llamadas — es decir, reconstruir a
mano lo que el servidor ya hace, en el punto exacto del que depende la seguridad del arnés.

**Gana A**, y no por poco. El control que B añade (samplers propios, logits) no lo necesita este
proyecto; el control que A da (gramáticas que garantizan la forma de la salida) encaja con su
filosofía: *el modelo no cumple porque quiera, cumple porque no puede hacer otra cosa*.

> **Pero hoy no lo estamos aprovechando.** La Tarea 13 fija `"temperature": 0.2` incrustada en el
> código y no manda ningún otro parámetro. Eso es lo contrario del control que se pide. Ver
> «Qué hay que corregir», más abajo.

## 2. Elegir, cambiar y actualizar el modelo

**A:** cambiar de modelo es cambiar la ruta del `.gguf` en `config.toml` y reiniciar el servidor
—que Programator ya arranca y cierra por su cuenta (§8.3 de la especificación)—. Sirve cualquier
GGUF que entienda tu `llama-server`. Probar tres modelos distintos en una tarde es editar una línea
tres veces.

**B:** cargar otro `.gguf` es igual de fácil… **hasta que el modelo nuevo es más moderno que tus
bindings**. Las arquitecturas nuevas llegan a `llama.cpp` continuamente; si el modelo que quieres
probar necesita una versión posterior a la que compilaste, no puedes usarlo hasta recompilar con un
binding actualizado. El modelo deja de ser una elección y pasa a ser una dependencia de compilación.

**Gana A, claramente.**

## 3. Actualizar el motor de inferencia

**A:** sustituir `llama-server.exe` por la versión nueva. No se toca ni una línea de Rust, no se
recompila nada, y volver atrás es restaurar el fichero anterior. Se puede tener la versión vieja y
la nueva y alternar.

**B:** subir la versión del crate y recompilar, lo que exige **clang y `bindgen`** en la máquina, más
el *toolkit* de CUDA para la variante de GPU. Y hay un detalle que sus propios autores advierten:
`llama-cpp-2` está deliberadamente pegado a `llama.cpp` y **no sigue semver de forma significativa**,
así que una actualización puede romper la compilación sin previo aviso. Tu ejecutable queda atado a
una versión concreta de `llama.cpp`.

**Gana A, con diferencia.** Este es el criterio donde B sale peor parado.

## 4. Rendimiento

Aquí la intuición engaña, así que conviene ponerlo en escala.

- El coste de una petición HTTP por `127.0.0.1` es del orden de **milisegundos**: no hay red, no hay
  DNS, no hay TLS; es una copia de memoria a través del bucle local.
- Generar una respuesta de un modelo de 24B en una GPU de 16 GB son **segundos o decenas de
  segundos**.
- Programator hace **una petición por turno de herramienta**, no una por token: no usa streaming.

El sobrecoste de A es, por tanto, ruido estadístico frente al tiempo de inferencia — muy por debajo
del 1 %. B se ahorra una serialización y una copia, es decir, ahorra exactamente donde no duele.

En ambos casos el modelo queda **residente en VRAM** entre ciclos, que es lo que de verdad importa:
el coste grande es cargar 24B, y se paga una sola vez.

**Empate práctico.** Quien elija B por rendimiento estará pagando un precio alto por una mejora que
no se puede medir en este caso de uso.

## 5. Defectos de cada opción

**Defectos de A, sin maquillar:**
- Un proceso extra que hay que arrancar, vigilar y cerrar.
- Un puerto local abierto. Si no se fija la interfaz de escucha, el riesgo de quedar expuesto a la
  red local depende del valor por defecto de un binario ajeno.
- Superficie de fallo propia del protocolo. **No es teórico: hoy mismo la revisión de la Tarea 13
  encontró que el cliente llamaba sin tiempo de espera de lectura**, de modo que un servidor que
  aceptara la conexión y no contestara dejaba el arnés colgado para siempre. Ya está corregido.
- Depender de que el servidor siga vivo (la especificación ya prevé reintentos con espera creciente).

**Defectos de B, sin maquillar:**
- Compilar C++ y CUDA en la máquina de desarrollo: clang, `bindgen`, toolkit. Adiós a «clonar y
  `cargo build`».
- El ejecutable deja de ser portable y pequeño: se lleva dentro el runtime de inferencia.
- Actualizar puede romper la compilación, por lo del semver.
- Hay que escribir a mano la plantilla de chat y el parseo de llamadas a herramientas: **más código
  delicado, justo en la pieza de la que depende el confinamiento**.
- **Aislamiento de fallos, que es el argumento más fuerte de todos:** si el modelo agota la VRAM y
  `llama.cpp` aborta, con A se muere el servidor y Programator lo detecta, lo publica en su buzón y
  reintenta; con B **se muere Programator entero**, a mitad de ciclo, con lo que eso implica para un
  proceso que corre desatendido.

---

## Recomendación

**Mantener A: `llama-server` con cliente HTTP en `127.0.0.1`.**

Gana en cuatro de los cinco criterios y empata en el quinto. Y gana precisamente en los dos que el
Director puso por delante —control efectivo y libertad para cambiar y actualizar el modelo—, además
de aislar los fallos del motor del proceso que debe sobrevivir a ellos.

B solo sería preferible si hiciera falta algo que únicamente da el acceso directo: un muestreador
propio, manipulación de logits, o control manual del KV cache entre llamadas. Nada de eso está en la
v1, ni en el horizonte de la v2.

### Qué hay que corregir para que esa recomendación valga

Tres cosas, y las dos primeras son defectos reales de la especificación descubiertos al hacer este
estudio:

1. **Fijar `--host 127.0.0.1` explícitamente** en la línea de arranque. Hoy el valor por defecto del
   servidor es ese, pero depender del valor por defecto de un binario externo para que el modelo no
   quede escuchando a toda la red local es una apuesta innecesaria: es una bandera.

2. **Añadir `--jinja`. Esto es grave.** La documentación del servidor dice que las llamadas a
   herramientas estilo OpenAI **requieren `--jinja`** (y a veces `--chat-template-file` para una
   plantilla compatible con herramientas). La línea de arranque que compone la Tarea 14 lleva
   `--model`, `--n-gpu-layers`, `--ctx-size`, `--flash-attn`, `--cache-type-k/v` y `--port`: **no
   lleva `--jinja`**. Tal como está, el modelo nunca emitiría `tool_calls`, el cliente de la Tarea 13
   solo vería texto, y **el repertorio confinado de la Tarea 11 no recibiría una sola solicitud**. Es
   decir: el arnés arrancaría y no funcionaría, y el fallo no aparecería hasta la primera prueba con
   GPU encendida.

3. **Sacar los parámetros de muestreo del código a `config.toml`.** Hoy la Tarea 13 lleva
   `"temperature": 0.2` incrustada y no manda nada más. Si se quiere el máximo control sobre el
   modelo, lo que hay que exponer es al menos `temperature`, `top_k`, `top_p`, `min_p`,
   `repeat_penalty` y `seed` —este último es el que da respuestas reproducibles—, dejando la puerta
   abierta a `grammar`/`json_schema` para forzar la forma de las llamadas a herramientas.

Los puntos 1 y 2 caen dentro de la Tarea 14, que es la siguiente y aún no se ha despachado. El punto
3 toca `config.toml` (Tarea 1) y el cliente (Tarea 13): es una ampliación de alcance y necesita el
visto bueno del Director.

---

## La decisión

**El Director adopta la opción A: `llama-server` como proceso aparte con cliente HTTP en
`127.0.0.1`.** Confirmada el 17/09/2026; el 16/09 ya había dicho que se sigue con `llama-server`
aparte y se empaqueta mejor, que es esta misma opción. Queda cerrado: el documento deja de estar
pendiente y **no hay que volver a plantear los bindings**.

Se descarta B (bindings en proceso) mientras no aparezca algo que solo dé el acceso directo —un
muestreador propio, manipulación de logits o control manual del caché KV entre llamadas—. Nada de
eso está en el horizonte.

**Los tres puntos correctivos que condicionaban la recomendación están hechos**, comprobado en el
código el 17/09/2026:

| Punto | Dónde está | Estado |
|---|---|---|
| 1. `--host 127.0.0.1` explícito | `src/motor/proceso.rs:30` | Hecho |
| 2. `--jinja` en la línea de arranque | `src/motor/proceso.rs:45`, con prueba que lo guarda en `:345` | Hecho |
| 3. Parámetros de muestreo en el TOML | `src/config.rs:185-222`, con `top_k`, `top_p` y `semilla` | Hecho |

El punto 2 era el grave: sin `--jinja` el modelo nunca emite `tool_calls` y el repertorio confinado
no recibe una sola solicitud. Hay una prueba que lo sujeta, así que no puede perderse en una
refactorización silenciosa.

**Lo que esta decisión no cierra.** Que el motor vaya aparte es lo decidido; *cómo se empaqueta y se
actualiza* es la 0.6.0 —fijar su versión en el TOML y descargar versiones nuevas solo cuando el
Director lo mande—, y *qué parámetros se exponen y cómo se reparte por tensores* es la 0.7.0. Ambas
construyen sobre esta decisión, no la reabren.

---

## Fuentes

- [Documentación del servidor de `llama.cpp`](https://github.com/ggml-org/llama.cpp/blob/master/tools/server/README.md)
  — parámetros de muestreo, gramáticas, requisito de `--jinja` para herramientas, y `127.0.0.1:8080`
  como enlace por defecto.
- [`llama-cpp-2` en crates.io](https://crates.io/crates/llama-cpp-2) y
  [`utilityai/llama-cpp-rs`](https://github.com/utilityai/llama-cpp-rs) — bindings de Rust: requieren
  clang y `bindgen`, se actualizan en sincronía con `llama.cpp` y no siguen semver de forma
  significativa.
- [`llama-cpp-sys-2` en crates.io](https://crates.io/crates/llama-cpp-sys-2) — bindings de bajo nivel
  con soporte de CUDA.
