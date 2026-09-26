# Pruebas manuales de Programator

Estas pruebas cubren lo que `cargo test` no puede: un diálogo nativo que necesita a una persona
delante, y (más adelante) un modelo real cargado en GPU. No forman parte de `cargo test`.

## Antes de empezar

1. Necesitas el binario `llama-server.exe` y el fichero `.gguf` del modelo descargados en tu
   máquina (solo hacen falta para las pruebas de la sección «Pendientes de la Tarea 16»; las
   pruebas A, B y C de aquí abajo no requieren GPU).
2. **Copia `programator.ejemplo.toml` a `programator.toml`, en la misma carpeta que
   `programator.exe`.** Sin este fichero, Programator no arranca: falla nombrando la ruta que
   buscó. Como mínimo tienes que rellenar `[motor] modelo = "..."` con la ruta a tu `.gguf`; el
   resto de campos ya trae valores razonables por defecto, comentados uno a uno en el propio
   fichero de ejemplo.
3. **Aviso sobre la carpeta de trabajo:** si `[carpeta] ruta` falta o está vacía en tu
   `programator.toml`, al arrancar Programator **se abre un diálogo nativo de Windows** pidiendo
   que elijas una carpeta, y el programa se queda esperando ahí hasta que eliges una o cancelas.
   Para las pruebas de sondeo (Prueba C) conviene fijar `ruta` de antemano y así no repetir el
   diálogo cada vez que reinicias el programa.

## Qué prueba hoy el binario, y qué no (léelo antes de seguir)

Tal como queda en la 0.5.0, `programator.exe` carga la configuración, resuelve la carpeta de
trabajo, instala las normas de la casa si hacen falta, crea `.gestor/canal/` si no existe, informa
siempre al arrancar de la GPU vista, el motor, el modelo y el encaje calculado, y entra en un bucle
que **sondea el canal, atiende los encargos y arranca `llama-server` cuando hace falta**. Las
Pruebas A, B, C y D de más abajo se pueden ejecutar hoy —los tres primeros pasos de la D no
requieren GPU ni modelo—. La sección final documenta las pruebas que sí necesitan GPU real, para que
no se pierda su criterio de aceptación.

## Prueba A — arranque, normas de la casa y estructura del canal

1. Crea una carpeta vacía cualquiera para usar como carpeta de trabajo de prueba.
2. En tu `programator.toml`, pon `[carpeta] ruta = "<ruta a esa carpeta>"`.
3. Ejecuta `programator.exe`.
4. **Señal de éxito:** en la consola aparece `Carpeta de trabajo: <tu ruta>` y, justo debajo,
   `Instaladas las normas de la casa en PROGRAMATOR.md` (esta segunda línea solo sale la primera
   vez; si vuelves a arrancar Programator sobre la misma carpeta, no debe repetirse).
5. Comprueba que la carpeta ahora tiene un fichero `PROGRAMATOR.md` con el contenido de
   `plantillas/normas-de-la-casa.md`.
6. Comprueba que se ha creado `<carpeta>/.gestor/canal/`.
7. Para el programa con **Ctrl+C** en la consola.

## Prueba B — el diálogo de carpeta cuando falta la configuración

1. Quita o vacía `[carpeta] ruta` en `programator.toml`.
2. Ejecuta `programator.exe`.
3. **Señal de éxito:** se abre un diálogo nativo titulado «Elige la carpeta de trabajo de
   Programator». Elige una carpeta cualquiera: el programa continúa igual que en la Prueba A.
4. Repite la prueba pero **cancela** el diálogo en vez de elegir una carpeta.
5. **Señal de éxito:** el programa termina de inmediato con un mensaje en la consola que empieza
   por `🔴 Programator no pudo arrancar:` y menciona que no se eligió carpeta de trabajo, y su
   código de salida es distinto de cero (`echo $LASTEXITCODE` en PowerShell tras ejecutarlo).

## Prueba C — el sondeo detecta novedades, incluidas las subcarpetas

1. Para agilizar la prueba, pon `[ciclo] intervalo_segundos = 5` en tu `programator.toml` (así no
   esperas los 180 segundos por defecto).
2. Ejecuta Programator sobre la carpeta de pruebas de la Prueba A.
3. Con el programa corriendo, añade un fichero directamente dentro de
   `<carpeta>/.gestor/canal/` (por ejemplo `codex.md`).
4. **Señal de éxito:** en menos de un intervalo aparece en consola
   `Novedades en el canal: atendiendo…`.
5. Repite añadiendo un fichero dentro de una **subcarpeta** del canal, por ejemplo
   `.gestor/canal/msg/001.md` (créala si no existe): también debe aparecer el aviso. Esto es
   importante porque el canal real del equipo tiene justo esa subcarpeta de mensajes.
6. Repite **borrando** ese mismo fichero: también debe detectarse.
7. Para el programa con Ctrl+C cuando termines.

## Prueba D — los cuatro comandos de la línea de órdenes

Cubre `--ayuda`, `--version`, `--diagnostico` y un argumento inventado, en ese orden. Las tres
primeras comprobaciones no requieren GPU ni modelo; la de `--diagnostico` con el informe completo sí
necesita la carpeta `portable/` montada con `llama-server` y el Devstral real
configurados, porque es la única forma de ver el encaje calculado contra hardware de verdad.

0. **Recompila y actualiza el binario de `portable/` antes de nada.** Ejecuta `cargo build
   --release` desde la raíz del paquete y copia `target/release/programator.exe` sobre
   `portable/programator.exe`. **Sin este paso estarías probando el binario que hubiera allí de
   antes:**
   ese binario ignora todos los argumentos de la línea de órdenes y entra directamente en el bucle
   de sondeo, así que con cualquiera de los cuatro comandos de aquí abajo verías el diálogo de
   carpeta, una sola línea de arranque y un bucle infinito —exactamente el defecto que esta versión
   corrige— sin poder distinguir si falló la 0.5.0 o si ejecutaste el binario viejo.
1. **`programator.exe --ayuda`** (y, por separado, `programator.exe -h`).
   **Señal de éxito:** en la consola sale un texto que empieza por `Programator 0.5.0 — arnés que
   convierte...`, incluye las cuatro formas de uso (`--ayuda`, `--version`, `--diagnostico` y la
   invocación sin argumentos) y termina limpiamente, sin arrancar ningún ciclo ni abrir el diálogo de
   carpeta. Las dos formas (`--ayuda` y `-h`) deben imprimir el mismo texto.
2. **`programator.exe --version`**.
   **Señal de éxito:** la única línea que aparece es `Programator 0.5.0`, y el programa termina de
   inmediato.
3. **`programator.exe --pamplinas`** (cualquier argumento inventado sirve).
   **Señal de éxito:** en la consola aparece exactamente
   `No entiendo «--pamplinas». Prueba «programator.exe --ayuda» para ver qué se le puede pedir a
   Programator.`, el programa **no** abre el diálogo de carpeta ni arranca el ciclo, y termina con
   código de salida 2 (`echo $LASTEXITCODE` en PowerShell tras ejecutarlo).
4. **`programator.exe --diagnostico`**, con `programator.toml` apuntando a la carpeta portable (el
   binario de `llama-server` con CUDA y el Devstral real en `[motor]`).
   **Qué tiene que ver el Director:** `--diagnostico` **no abre ningún diálogo**, tenga o no fijada
   `[carpeta] ruta` en el TOML: la consola imprime el informe completo directamente y nada de bucle
   detrás (el programa termina solo, sin esperar Ctrl+C), con estas cinco líneas:
   - `Carpeta de trabajo:` con la ruta fijada en el TOML, o, si no la tiene, el texto que dice que no
     está fijada y que se elegiría con un diálogo al arrancar de verdad (con `--diagnostico` nunca se
     llega a abrir).
   - `GPU:` con el nombre de la tarjeta y la VRAM libre y total en GiB.
   - `Motor:` diciendo `llama-server.exe con CUDA, en <ruta de «motor.binario» en el TOML>` (no `SIN
     CUDA`: la carpeta portable trae el motor compilado con `ggml-cuda.dll`).
   - `Modelo:` nombrando el fichero del Devstral, **con «40 capas»** y su peso en GiB.
   - `Encaje:` con el aviso que redacta `motor::encaje::redactar_aviso` (no un texto fijo: si el
     modelo cabe entero dice algo del tipo «el modelo entero cabe en `<GPU>` (`<libres>` libres): sus
     40 capas piden `<tanto>` y van a la GPU»; si no cabe entero, dice cuántas de las 40 van a la GPU
     y cuántas se quedan en CPU), seguido siempre de «; se pedirán N capas» con las que de verdad se
     le pasarán a `llama-server`. Nunca 0 capas con VRAM de sobra ni más de 40 (41 contando la capa
     de salida aparte).

   **Si el informe dice otra cosa —el motor sale SIN CUDA, el modelo no aparece con sus 40 capas, o
   el encaje no cuadra con lo de arriba—, hay que parar y no dar la verificación por buena**: es la
   comprobación pendiente de esta versión contra la GPU real, y hasta que se ejecute con éxito el
   cálculo del encaje solo se ha visto probado en frío.

## Prueba E — el peso real del modelo cuadra con el fichero, y el aviso de encaje es coherente

Requiere el Devstral real descargado en `modelos/devstral-small-2-24b-Q4_K_M.gguf` —el mismo
nombre de fichero que fija `programator.ejemplo.toml`— (ruta relativa a la raíz del paquete, la
misma desde la que corre `cargo test`; no el GGUF de juguete de las pruebas automáticas) y una GPU
dedicada en la máquina, así que no forma parte de la suite normal.

1. `src/motor/gguf/mod.rs` deja puesta, marcada `#[ignore]` igual que la sonda de
   `motor::hardware`, la prueba `el_peso_del_devstral_real_cuadra_con_el_tamano_del_fichero`: pesa
   `modelos/devstral-small-2-24b-Q4_K_M.gguf` con `leer_modelo` y comprueba que
   `inicio_datos + fin_datos` se queda a menos de una alineación por debajo del tamaño real del
   fichero —no exactamente igual: un escritor GGUF real rellena hasta la alineación también
   después del último tensor, así que sobran entre 1 y 31 bytes que la suma no cuenta—. Ejecútala
   con:

   ```
   cargo test --lib motor::gguf -- --ignored --nocapture el_peso_del_devstral_real_cuadra_con_el_tamano_del_fichero
   ```

   **Señal de éxito:** `test result: ok` y, en la salida (visible gracias a `--nocapture`), una
   línea `inicio_datos + fin_datos = ... | tamaño del fichero = ...` con la suma igual o unos
   pocos bytes por debajo del tamaño del fichero (menos de 32, la alineación habitual). Si el
   `assert!` falla porque la diferencia es mayor o la suma supera al fichero, algún tamaño de la
   tabla de tipos (`motor::tipos_tensor`) está mal. Si el fichero no está en el equipo, la prueba
   lo dice por consola y pasa sin comprobar nada: no es una señal de éxito, es que no se pudo
   ejecutar.
2. Arranca `programator.exe` con ese modelo configurado en `[motor] modelo`. **Señal de éxito:** la
   consola imprime una línea `Encaje en la GPU: ...` antes de lanzar `llama-server`, y el número de
   capas que menciona es coherente con las 40 que tiene Devstral —«las 40 caben», «40 de 40 más la
   salida», o un reparto parcial con menos de 41—, nunca 0 con una GPU con VRAM de sobra ni un
   número mayor que 41. También es una degradación legítima, y no un fallo, que el aviso caiga a
   las 99 capas de siempre si no pudo medir la GPU o pesar el modelo: el §8 de la especificación lo
   contempla como desenlace correcto, no como avería.

---

## Pendientes de la Tarea 16 (requieren GPU; no ejecutables hoy)

Las siguientes tres pruebas están documentadas para no perder el criterio de aceptación original. El
ciclo que necesitan ya está encadenado —`programator.exe` arranca `llama-server` y llama al modelo
desde la 0.3.0, ver la sección de arriba—: lo que falta no es código, es ejecutarlas con motor y
modelo reales sobre una GPU dedicada. Repásalas antes de darlas por buenas si algo de lo descrito
cambia.

### 1. El motor arranca y responde

1. Coloca `llama-server.exe` en `herramientas/` y el `.gguf` del modelo en `modelos/`, y comprueba
   que tu `programator.toml` los referencia con esas rutas.
2. Ejecuta `programator.exe`.
3. **Dónde mirar la salida:** `llama-server` no escribe ningún fichero de registro. Todo lo que
   imprime sale **en la misma consola donde corre Programator**, mezclado con sus propios mensajes
   (`Carpeta de trabajo: ...`, `Novedades en el canal: ...`). No busques un log en disco: no existe.
4. **Señal de que el servidor está arriba:** el texto exacto que imprime `llama-server` al terminar
   de cargar varía entre versiones, así que no lo uses como única señal. La comprobación objetiva es
   que el puerto configurado (8080 por defecto) empiece a aceptar conexiones TCP: desde otra
   consola, `Test-NetConnection -ComputerName 127.0.0.1 -Port 8080` (PowerShell) debe devolver
   `TcpTestSucceeded : True`. Como apoyo visual, comprueba también en el administrador de tareas que
   el proceso `llama-server` aparece y que la VRAM sube varios GB al cargar el modelo.
5. **Para cerrar y comprobar que no queda huérfano:** cierra Programator y comprueba en el
   administrador de tareas (o `Get-Process llama-server -ErrorAction SilentlyContinue` en
   PowerShell) que el proceso `llama-server` ha desaparecido. **Lee el aviso de más abajo antes de
   dar esta prueba por buena.**

### 2. Un ciclo completo sobre una copia del canal

1. Copia `.gestor/canal/` de MMCelt a una carpeta de pruebas.
2. Añade a `codex.md` una sección `## Para Programator` con un encargo sencillo.
3. Ejecuta Programator sobre esa carpeta.
4. Comprueba en `programator.md` que la respuesta lleva `LATIDO:` y `LEÍDO:`.
5. Comprueba con una huella SHA-256 que `codex.md` y `claude.md` **no han cambiado**.

### 3. Coste por ciclo

1. Deja Programator dos ciclos sin trabajo nuevo.
2. Localiza en la consola (mezclado con la salida de Programator, ver punto 3 de la prueba
   anterior) lo que `llama-server` reporta por petición, y comprueba que los tokens de entrada por
   ciclo se mantienen por debajo de 3.000.

## Prueba F — el motor arranca con un solo slot y el ciclo va a la velocidad que debe

**Por qué existe.** Es la prueba que habría ahorrado la mitad del rendimiento durante cinco
versiones. Ninguna prueba automática puede hacerla: hace falta la tarjeta encendida y el modelo
cargado.

**Paso 0.** Recompilar y copiar el ejecutable, o estarás probando la versión anterior:

```
cargo build --release
copy target
elease\programator.exe portable```

**Paso 1.** Comprobar lo que ofrece el encaje:

```
portable\programator.exe --diagnostico
```

Con el contexto por defecto (16384) y una RTX 4070 Ti SUPER con la tarjeta libre, debe ofrecer
**39 de 40 capas**. Si ofrece 35, el contexto sigue en 32768: míralo en tu `programator.toml`.

**Paso 2.** Arrancar el ciclo, publicar un encargo cualquiera y mirar la **primera línea** que
escribe el motor al cargar:

```
srv load_model: initializing, n_slots = 1, n_ctx_slot = 16384
```

**`n_slots = 1` es lo que hay que ver.** Si dice 4, `--parallel` no está llegando al motor y el
modelo está perdiendo capas en la CPU sin que nada lo avise.

**Paso 3.** En el mismo registro, buscar el tiempo de generación del encargo:

```
slot print_timing: ... eval time = ... ( ... ms per token, 18.35 tokens per second)
```

**Lo esperable son 18 tokens por segundo o más.** Si está cerca de 10, hay capas en la CPU que no
deberían estar ahí. La medida de referencia del 22/09/2026 sobre el Devstral: 9,8 t/s con la
configuración de la 0.5.0, 18,4 t/s con la de la 0.6.0, y 23,0 t/s con las 40 capas en la tarjeta.

---

## Prueba G — el primer encargo de la sesión se atiende solo

**Por qué existe.** Es el defecto INC-04, y ninguna prueba automática puede cubrirlo: hace falta un
motor de verdad tardando de verdad en cargar. Hasta la 0.7.1, **el primer encargo de cada sesión se
quedaba colgado para siempre** si nadie volvía a escribir en el canal.

**Paso 1.** Asegurarse de que **no hay ningún `llama-server` corriendo**. Es la clave de la prueba:
se trata de que Programator arranque el suyo y lo pille cargando.

**Paso 2.** Arrancar `portable\programator.exe` y esperar a ver el informe de arranque.

**Paso 3.** Publicar **un solo encargo** en el canal, como sección nueva `## Para Programator: …`.

**Paso 4.** No tocar nada más. Ni el canal, ni los ficheros, nada.

**Lo que debe verse**, en este orden:

```
Novedades en el canal: atendiendo…
Arrancando el motor — encaje: …
El motor local todavía está cargando el modelo: se reintenta en el ciclo siguiente.
Se retoma el encargo que quedó esperando a que cargara el modelo.
Encargos atendidos: 1 (fallos al publicar: 0).
```

**La línea que importa es la cuarta.** Si tras «todavía está cargando» no aparece nada más y el
programa se queda mudo, INC-04 ha vuelto.

---

## Prueba H — Programator no presupone el proyecto (0.9.0)

**Por qué existe.** Ninguna prueba automática puede comprobar que el arnés no ensucia un repositorio
ajeno ni que arranca sin una persona delante: hacen falta una carpeta de proyecto de verdad y dos
arranques seguidos. Es lo que falló la jornada entera del 23/09/2026 en NatureLand.

**Paso 0, y no se salta.** Copiar el ejecutable recién compilado sobre el portable:

```
cargo build --release
copy target\release\programator.exe portable\programator.exe
```

Probar con un binario viejo da síntomas desconcertantes.

**Paso 1 — arranca sin diálogo.** Con una carpeta de proyecto cualquiera que no sea la habitual:

```
portable\programator.exe --ruta "C:\ruta\a\otro\proyecto"
```

Debe entrar en el ciclo **sin abrir ningún diálogo** y escribir en la consola una línea
«Carpeta de trabajo: … — pedida en la línea de órdenes».

**Paso 2 — no ensucia la raíz.** Mirar la raíz de esa carpeta: **no puede haber ningún fichero
nuevo**. Las instrucciones tienen que estar en `.gestor\programator\PROGRAMATOR.md`, junto a
`lectura.json`.

**Paso 3 — las instrucciones dicen la verdad.** Abrir ese `PROGRAMATOR.md` e ir a la sección «Cómo
se comprueba lo que entregas». Tiene que nombrar **exactamente** los comprobadores que haya en
`[verificacion.comprobadores]` del TOML en vigor, y ninguno más. Si no hay ninguno configurado, debe
decir que no hay ninguno, no callarse. **En ningún caso puede aparecer `cargo` si el proyecto no es
de Rust.**

**Paso 4 — se acuerda de la carpeta.** Cerrar y volver a arrancar, esta vez **sin `--ruta`**, con
`[carpeta] ruta` comentada en el TOML y con `[carpeta] preguntar_siempre = false`. Debe ir a la
misma carpeta del paso 1, sin abrir el diálogo, y decir «la última en la que se trabajó». El fichero
`portable\ultima-carpeta.txt` debe contener esa ruta.

Después, poner `preguntar_siempre = true` (que es el valor de fábrica) y arrancar otra vez igual:
ahora **sí** tiene que abrirse el diálogo, aunque el fichero de memoria siga ahí con la ruta buena.

**Paso 5 — la ventana declarada es la real.** Abrir
`.gestor\canal\COMO-ENCARGAR-A-PROGRAMATOR.md` y comprobar que la cifra de tokens que declara es la
misma que `[motor] contexto` del TOML en vigor. Para asegurarse de que no es coincidencia, cambiar
`contexto` en el TOML, rearrancar y ver que la guía cambia con él.

**Paso 6 — la línea del canal no engaña.** Encargarle un fichero de una extensión que **no** tenga
comprobador configurado. Cuando entregue, su buzón debe decir «⚠️ SIN COMPROBAR — no hay comprobador
configurado para «.xx»», nombrando la extensión. Si dice «sin verificar», la 0.9.0 ha retrocedido.

**Paso 7 — no rompe lo que ya estaba.** En una carpeta que ya tenga su `PROGRAMATOR.md` **en la
raíz** (MMCelt lo tiene), arrancar y comprobar que ese fichero **se sigue usando, no se modifica y
no se crea un segundo** en `.gestor\programator\`.

---

### Aviso para quien implemente la Tarea 16: el cierre con Ctrl+C puede no bastar

`ServidorLocal` (Tarea 14, `src/motor/proceso.rs`) mata a `llama-server` en su `Drop`, pero **`Drop`
solo se ejecuta si el proceso termina de forma ordenada** (la función `main` devuelve, o hay un
`panic!` con *unwind*). En Windows, si se detiene un programa de consola con **Ctrl+C** y el
programa no ha registrado su propio manejador de consola, el comportamiento por defecto del sistema
es terminar el proceso en el acto sin ejecutar destructores. Hoy el proyecto no trae ningún
manejador de Ctrl+C (ni la crate `ctrlc` ni un manejador de consola propio) — comprobado revisando
`Cargo.toml` y todo `src/`, no aparece ninguno —, así que **cerrar Programator con Ctrl+C podría
dejar `llama-server` huérfano** en cuanto exista esa integración. La Prueba 1 de arriba debe
repetirse cerrando específicamente con Ctrl+C, no solo cerrando la ventana de la consola, antes de
dar la Tarea 16 por buena.
