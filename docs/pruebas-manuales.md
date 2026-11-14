Pruebas manuales de Programator

Estas pruebas cubren lo que cargo test no puede: un diálogo nativo que necesita a una persona delante, y (más adelante) un modelo real cargado en GPU. No forman parte de cargo test.


Antes de empezar

Primero, necesitas el binario llama-server.exe y el fichero .gguf del modelo descargados en tu máquina. Solo hacen falta para las pruebas de la sección «Pendientes (requieren GPU)»; las pruebas A, B y C de aquí abajo no requieren GPU.

Segundo, copia programator.ejemplo.toml a programator.toml, en la misma carpeta que programator.exe. Sin este fichero, Programator no arranca: falla nombrando la ruta que buscó. Como mínimo tienes que rellenar «[motor] modelo = "..."» con la ruta a tu .gguf; el resto de campos ya trae valores razonables por defecto, comentados uno a uno en el propio fichero de ejemplo.

Tercero, un aviso sobre la carpeta de trabajo: si «[carpeta] ruta» falta en tu programator.toml, al arrancar Programator se abre un diálogo nativo de Windows pidiendo que elijas una carpeta, y el programa se queda esperando ahí hasta que eliges una o cancelas. Así ocurre con «[carpeta] preguntar_siempre = true», que es el valor de fábrica; con false, va sin preguntar a la última carpeta en la que se trabajó, si la recuerda. Para las pruebas de sondeo (Prueba C) conviene fijar ruta de antemano y así no repetir el diálogo cada vez que reinicias el programa.


Qué prueba hoy el binario, y qué no (léelo antes de seguir)

Tal como queda en la 0.12.0, programator.exe carga la configuración, resuelve la carpeta de trabajo, instala sus instrucciones (PROGRAMATOR.md) si hacen falta, crea .gestor/canal/ si no existe, informa siempre al arrancar de la GPU vista, el motor, el modelo y el encaje calculado, y entra en un bucle que sondea el canal, atiende los encargos y arranca llama-server cuando hace falta. Las Pruebas A, B, C y D de más abajo se pueden ejecutar sin GPU (los tres primeros pasos de la D no requieren GPU ni modelo). La sección «Pendientes (requieren GPU)» y las pruebas de la E en adelante documentan las que necesitan motor y modelo reales o una máquina de pruebas, para que no se pierda su criterio de aceptación.


Prueba A — arranque, normas de la casa y estructura del canal

Primero, crea una carpeta vacía cualquiera para usar como carpeta de trabajo de prueba. Después, en tu programator.toml, pon «[carpeta] ruta = "<ruta a esa carpeta>"». A continuación, ejecuta programator.exe.

La señal de éxito es que en la consola aparece «Carpeta de trabajo: <tu ruta> — fijada en programator.toml» y, justo debajo, «Instaladas las instrucciones del proyecto en <tu ruta>\.gestor\programator\PROGRAMATOR.md». Esta segunda línea solo sale la primera vez; si vuelves a arrancar Programator sobre la misma carpeta, no debe repetirse.

Comprueba entonces que la carpeta tiene ahora un fichero .gestor/programator/PROGRAMATOR.md, compuesto a partir de plantillas/instrucciones-del-proyecto.md, y que se ha creado <carpeta>/.gestor/canal/. Por último, para el programa con Ctrl+C en la consola.


Prueba B — el diálogo de carpeta cuando falta la configuración

Primero, quita o comenta «[carpeta] ruta» en programator.toml. Después, ejecuta programator.exe. La señal de éxito es que se abre un diálogo nativo titulado «Elige la carpeta de trabajo de Programator». Elige una carpeta cualquiera: el programa continúa igual que en la Prueba A.

Luego repite la prueba, pero cancela el diálogo en vez de elegir una carpeta. La señal de éxito es que el programa termina de inmediato con un mensaje en la consola que empieza por «🔴 Programator no pudo arrancar:» y menciona que no se eligió carpeta de trabajo, y que su código de salida es distinto de cero (se ve con echo $LASTEXITCODE en PowerShell tras ejecutarlo).


Prueba C — el sondeo detecta novedades, incluidas las subcarpetas

Primero, para agilizar la prueba, pon «[ciclo] intervalo_segundos = 5» en tu programator.toml; así no esperas los 180 segundos por defecto. Después, ejecuta Programator sobre la carpeta de pruebas de la Prueba A. Con el programa corriendo, añade un fichero directamente dentro de <carpeta>/.gestor/canal/, por ejemplo codex.md. La señal de éxito es que en menos de un intervalo aparece en la consola «Novedades en el canal: atendiendo…».

Repite añadiendo un fichero dentro de una subcarpeta del canal, por ejemplo .gestor/canal/msg/001.md (créala si no existe): también debe aparecer el aviso. Esto es importante porque el canal real del equipo tiene justo esa subcarpeta de mensajes. Repite después borrando ese mismo fichero: también debe detectarse. Cuando termines, para el programa con Ctrl+C.


Prueba D — los cuatro comandos de la línea de órdenes

Cubre --ayuda, --version, --diagnostico y un argumento inventado, en ese orden. Las tres primeras comprobaciones no requieren GPU ni modelo; la de --diagnostico con el informe completo sí necesita la carpeta portable/ montada con llama-server y el Devstral real configurados, porque es la única forma de ver el encaje calculado contra hardware de verdad.

Antes de nada, recompila y actualiza el binario de portable/. Ejecuta cargo build --release desde la raíz del paquete y copia target/release/programator.exe sobre portable/programator.exe. Sin este paso estarías probando el binario que hubiera allí de antes, sin poder distinguir si falló la versión nueva o si ejecutaste el binario viejo.

Primero, pide la ayuda, y por separado su forma corta:

    programator.exe --ayuda
    programator.exe -h

La señal de éxito es que en la consola sale un texto que empieza por «Programator 0.12.0 — arnés que convierte...», incluye las formas de uso, entre ellas --ayuda, --version, --diagnostico y la invocación sin argumentos, y termina limpiamente, sin arrancar ningún ciclo ni abrir el diálogo de carpeta. Las dos formas, --ayuda y -h, deben imprimir el mismo texto.

Segundo, pide la versión:

programator.exe --version

La señal de éxito es que la única línea que aparece es «Programator 0.12.0», y el programa termina de inmediato.

Tercero, pásale un argumento inventado; cualquiera sirve:

programator.exe --pamplinas

La señal de éxito es que en la consola aparece exactamente este texto:

No entiendo «--pamplinas». Prueba «programator.exe --ayuda» para ver qué se le puede pedir a Programator.

Además, el programa no abre el diálogo de carpeta ni arranca el ciclo, y termina con código de salida 2 (se ve con echo $LASTEXITCODE en PowerShell tras ejecutarlo).

Cuarto, pide el diagnóstico, con programator.toml apuntando a la carpeta portable (el binario de llama-server con CUDA y el Devstral real en [motor]):

programator.exe --diagnostico

Qué tiene que verse: --diagnostico no abre ningún diálogo, tenga o no fijada «[carpeta] ruta» en el TOML. La consola imprime el informe completo directamente y nada de bucle detrás (el programa termina solo, sin esperar Ctrl+C), con estas cinco líneas.

La línea «Carpeta de trabajo:» lleva la ruta fijada en el TOML o, si no la tiene, el texto que dice que no está fijada y que se elegiría con un diálogo al arrancar de verdad (con --diagnostico nunca se llega a abrir).

La línea «GPU:» lleva el nombre de la tarjeta y la VRAM libre y total en GiB.

La línea «Motor:» dice «llama-server con CUDA, en <ruta de «motor.binario» en el TOML>», y no «SIN CUDA»: la carpeta portable trae el motor compilado con ggml-cuda.dll.

La línea «Modelo:» nombra el fichero del Devstral, con «40 capas», y su peso en GiB.

La línea «Encaje:» lleva el aviso que redacta motor::encaje::redactar_aviso. No es un texto fijo: si el modelo cabe entero dice algo del tipo «el modelo entero cabe en <GPU> (<libres> libres): sus 40 capas piden <tanto> y van a la GPU»; si no cabe entero, dice cuántas de las 40 van a la GPU y cuántas se quedan en CPU. Siempre va seguido de «; se pedirán N capas», con las que de verdad se le pasarán a llama-server. Nunca 0 capas con VRAM de sobra ni más de 40 (41 contando la capa de salida aparte).

Si el informe dice otra cosa (el motor sale SIN CUDA, el modelo no aparece con sus 40 capas, o el encaje no cuadra con lo de arriba), hay que parar y no dar la verificación por buena: es la comprobación pendiente de esta versión contra la GPU real, y hasta que se ejecute con éxito el cálculo del encaje solo se ha visto probado en frío.


Prueba E — el peso real del modelo cuadra con el fichero, y el aviso de encaje es coherente

Requiere el Devstral real descargado en modelos/devstral-small-2-24b-Q4_K_M.gguf, el mismo nombre de fichero que fija programator.ejemplo.toml (ruta relativa a la raíz del paquete, la misma desde la que corre cargo test; no el GGUF de juguete de las pruebas automáticas), y una GPU dedicada en la máquina, así que no forma parte de la suite normal.

Primero, el peso. El fichero src/motor/gguf/mod.rs deja puesta, marcada #[ignore] igual que la sonda de motor::hardware, la prueba el_peso_del_devstral_real_cuadra_con_el_tamano_del_fichero. Esa prueba pesa modelos/devstral-small-2-24b-Q4_K_M.gguf con leer_modelo y comprueba que inicio_datos + fin_datos se queda a menos de una alineación por debajo del tamaño real del fichero. No exactamente igual: un escritor GGUF real rellena hasta la alineación también después del último tensor, así que sobran entre 1 y 31 bytes que la suma no cuenta. Ejecútala con:

cargo test --lib motor::gguf -- --ignored --nocapture el_peso_del_devstral_real_cuadra_con_el_tamano_del_fichero

La señal de éxito es «test result: ok» y, en la salida (visible gracias a --nocapture), una línea «inicio_datos + fin_datos = ... | tamaño del fichero = ...» con la suma igual o unos pocos bytes por debajo del tamaño del fichero (menos de 32, la alineación habitual). Si el assert! falla porque la diferencia es mayor o la suma supera al fichero, algún tamaño de la tabla de tipos (motor::tipos_tensor) está mal. Si el fichero no está en el equipo, la prueba lo dice por consola y pasa sin comprobar nada: no es una señal de éxito, es que no se pudo ejecutar.

Después, arranca programator.exe con ese modelo configurado en «[motor] modelo». La señal de éxito es que la consola imprime una línea «Arrancando el motor — encaje: ...» antes de lanzar llama-server, y que el número de capas que menciona es coherente con las 40 que tiene Devstral («sus 40 capas piden ... y van a la GPU», «caben N de 40 capas», o un reparto parcial con menos de 41), nunca 0 con una GPU con VRAM de sobra ni un número mayor que 41. También es una degradación legítima, y no un fallo, que el aviso caiga a las 99 capas de siempre si no pudo medir la GPU o pesar el modelo: el §8 de la especificación lo contempla como desenlace correcto, no como avería.


Pendientes (requieren GPU)

Las siguientes tres pruebas están documentadas para no perder el criterio de aceptación original. El ciclo que necesitan ya está encadenado (programator.exe arranca llama-server y llama al modelo desde la 0.3.0, ver la sección de arriba): lo que falta no es código, es ejecutarlas con motor y modelo reales sobre una GPU dedicada. Repásalas antes de darlas por buenas si algo de lo descrito cambia.


Prueba pendiente 1: el motor arranca y responde

Primero, coloca llama-server.exe en herramientas/ y el .gguf del modelo en modelos/, y comprueba que tu programator.toml los referencia con esas rutas. Después, ejecuta programator.exe.

Dónde mirar la salida: llama-server no escribe ningún fichero de registro. Todo lo que imprime sale en la misma consola donde corre Programator, mezclado con sus propios mensajes («Carpeta de trabajo: ...», «Novedades en el canal: ...»). No busques un log en disco: no existe.

Señal de que el servidor está arriba: el texto exacto que imprime llama-server al terminar de cargar varía entre versiones, así que no lo uses como única señal. La comprobación objetiva es que el puerto configurado (8080 por defecto) empiece a aceptar conexiones TCP. Desde otra consola, en PowerShell, ejecuta:

Test-NetConnection -ComputerName 127.0.0.1 -Port 8080

Debe devolver «TcpTestSucceeded : True». Como apoyo visual, comprueba también en el administrador de tareas que el proceso llama-server aparece y que la VRAM sube varios GB al cargar el modelo.

Para cerrar y comprobar que no queda huérfano: cierra Programator y comprueba en el administrador de tareas que el proceso llama-server ha desaparecido. También se puede comprobar en PowerShell con:

Get-Process llama-server -ErrorAction SilentlyContinue

Lee el aviso de más abajo antes de dar esta prueba por buena.


Prueba pendiente 2: un ciclo completo sobre una copia del canal

Primero, copia el .gestor/canal/ de un proyecto real a una carpeta de pruebas. Después, añade a codex.md una intervención nueva, «LATIDO: <fecha y hora>», y debajo una línea «Para Programator: <un encargo sencillo>». A continuación, ejecuta Programator sobre esa carpeta.

Comprueba en programator.md que la respuesta empieza con «LATIDO: AAAA-MM-DD HH:MM:SS», sin negrita, y lleva «LEÍDO:». Si el modelo reservó algo, la línea ya no está en estado.md. Por último, comprueba con una huella SHA-256 que codex.md y claude.md no han cambiado.


Prueba pendiente 3: coste por ciclo

Primero, deja Programator dos ciclos sin trabajo nuevo. Después, localiza en la consola (mezclado con la salida de Programator, ver «Dónde mirar la salida» en la prueba pendiente 1) lo que llama-server reporta por petición, y comprueba que los tokens de entrada por ciclo se mantienen por debajo de 3.000.


Prueba F — el motor arranca con un solo slot y el ciclo va a la velocidad que debe

Por qué existe. Es la prueba que habría ahorrado la mitad del rendimiento durante cinco versiones. Ninguna prueba automática puede hacerla: hace falta la tarjeta encendida y el modelo cargado.

Paso 0. Recompilar y copiar el ejecutable, o estarás probando la versión anterior:

    cargo build --release
    copy target\release\programator.exe portable\programator.exe

Paso 1. Comprobar lo que ofrece el encaje:

portable\programator.exe --diagnostico

Con el contexto por defecto (16384) y una RTX 4070 Ti SUPER con la tarjeta libre, debe ofrecer 39 de 40 capas. Si ofrece 35, el contexto sigue en 32768: míralo en tu programator.toml.

Paso 2. Arrancar el ciclo, publicar un encargo cualquiera y mirar la primera línea que escribe el motor al cargar:

srv load_model: initializing, n_slots = 1, n_ctx_slot = 16384

Lo que hay que ver es n_slots = 1. Si dice 4, --parallel no está llegando al motor y el modelo está perdiendo capas en la CPU sin que nada lo avise.

Paso 3. En el mismo registro, buscar el tiempo de generación del encargo:

slot print_timing: ... eval time = ... ( ... ms per token, 18.35 tokens per second)

Lo esperable son 18 tokens por segundo o más. Si está cerca de 10, hay capas en la CPU que no deberían estar ahí. La medida de referencia del 22/09/2026 sobre el Devstral: 9,8 t/s con la configuración de la 0.5.0, 18,4 t/s con la de la 0.6.0, y 23,0 t/s con las 40 capas en la tarjeta.


Prueba G — el primer encargo de la sesión se atiende solo

Por qué existe. Es un defecto que ya se dio, y ninguna prueba automática puede cubrirlo: hace falta un motor de verdad tardando de verdad en cargar. Hasta la 0.7.1, el primer encargo de cada sesión se quedaba colgado para siempre si nadie volvía a escribir en el canal.

Paso 1. Asegurarse de que no hay ningún llama-server corriendo. Es la clave de la prueba: se trata de que Programator arranque el suyo y lo pille cargando.

Paso 2. Arrancar portable\programator.exe y esperar a ver el informe de arranque.

Paso 3. Publicar un solo encargo en el canal, en una intervención nueva con su línea «Para Programator: …».

Paso 4. No tocar nada más. Ni el canal, ni los ficheros, nada.

Lo que debe verse, en este orden:

    Novedades en el canal: atendiendo…
    Arrancando el motor — encaje: …
    El motor local todavía está cargando el modelo: se reintenta en el ciclo siguiente.
    Se retoma el encargo que quedó esperando a que cargara el modelo.
    Encargos atendidos: 1 (fallos al publicar: 0).

La línea que importa es la cuarta. Si tras «todavía está cargando» no aparece nada más y el programa se queda mudo, el defecto ha vuelto.


Prueba H — Programator no presupone el proyecto (0.9.0)

Por qué existe. Ninguna prueba automática puede comprobar que el arnés no ensucia un repositorio ajeno ni que arranca sin una persona delante: hacen falta una carpeta de proyecto de verdad y dos arranques seguidos. Es lo que falló la jornada entera del 23/09/2026 en uso real.

Paso 0, y no se salta. Copiar el ejecutable recién compilado sobre el portable:

    cargo build --release
    copy target\release\programator.exe portable\programator.exe

Probar con un binario viejo da síntomas desconcertantes.

Paso 1: arranca sin diálogo. Con una carpeta de proyecto cualquiera que no sea la habitual:

portable\programator.exe --ruta "C:\ruta\a\otro\proyecto"

Debe entrar en el ciclo sin abrir ningún diálogo y escribir en la consola una línea «Carpeta de trabajo: … — pedida en la línea de órdenes».

Paso 2: no ensucia la raíz. Mirar la raíz de esa carpeta: no puede haber ningún fichero nuevo. Las instrucciones tienen que estar en .gestor\programator\PROGRAMATOR.md, junto a lectura.json.

Paso 3: las instrucciones dicen la verdad. Abrir ese PROGRAMATOR.md e ir a la sección «Cómo se comprueba lo que entregas». Tiene que nombrar exactamente los comprobadores que haya en [verificacion.comprobadores] del TOML en vigor, y ninguno más. Si no hay ninguno configurado, debe decir que no hay ninguno, no callarse. En ningún caso puede aparecer cargo si el proyecto no es de Rust.

Paso 4: se acuerda de la carpeta. Cerrar y volver a arrancar, esta vez sin --ruta, con «[carpeta] ruta» comentada en el TOML y con «[carpeta] preguntar_siempre = false». Debe ir a la misma carpeta del paso 1, sin abrir el diálogo, y decir «la última en la que se trabajó». El fichero portable\ultima-carpeta.txt debe contener esa ruta.

Después, poner «preguntar_siempre = true» (que es el valor de fábrica) y arrancar otra vez igual: ahora sí tiene que abrirse el diálogo, aunque el fichero de memoria siga ahí con la ruta buena.

Paso 5: la ventana declarada es la real. Abrir .gestor\canal\COMO-ENCARGAR-A-PROGRAMATOR.md y comprobar que la cifra de tokens que declara es la misma que «[motor] contexto» del TOML en vigor. Para asegurarse de que no es coincidencia, cambiar contexto en el TOML, rearrancar y ver que la guía cambia con él.

Paso 6: la línea del canal no engaña. Encargarle un fichero de una extensión que no tenga comprobador configurado. Cuando entregue, su buzón debe decir «⚠️ SIN COMPROBAR — no hay comprobador configurado para «.xx»», nombrando la extensión. Si dice «sin verificar», la 0.9.0 ha retrocedido.

Paso 7: no rompe lo que ya estaba. En una carpeta que ya tenga su PROGRAMATOR.md en la raíz, arrancar y comprobar que ese fichero se sigue usando, no se modifica y no se crea un segundo en .gestor\programator\.


Prueba I — el banco de modelos mide lo que dice

Por qué existe. Las pruebas automáticas del banco pasan los encargos con un motor de mentira y lanzan las pruebas ocultas contra un lanzador de mentira. Solo esta prueba junta el banco con el motor real, con las pruebas ocultas de verdad y con una persona que puntúa a mano.

Antes de nada, dónde. Esta prueba ocupa la tarjeta durante horas y ejecuta código del modelo en el equipo donde corre Programator: Programator no usa ninguna otra máquina. Conviene hacerla en una máquina de pruebas, nunca en el equipo de uso diario, y dejarla limpia al acabar.

Paso 0: binario nuevo y configuración. Como en la prueba H, copiar el ejecutable recién compilado sobre el portable. Copiar evaluacion\banco\banco.ejemplo.toml a evaluacion\banco\banco.toml y descomentar la sección [comprobacion_local], con los nombres que pida ese sistema. Comprobar con --diagnostico que el modelo cabe entero en la VRAM libre: si no cabe, la velocidad medida no sirve de referencia.

Paso 1: la tanda. Desde la raíz del repositorio:

portable\programator.exe --banco evaluacion\banco

Antes de empezar debe decir cuántos encargos y semillas va a pasar, cuánto calcula que tardará y que el código del modelo se ejecutará en este equipo, dentro de la carpeta comprobacion de la tanda. No pide ninguna autorización. Mientras dura, una línea por encargo.

Paso 1b: si fallan las pruebas ocultas, no se repite la tanda. Si la fase 2 falla, el banco tiene que decir dónde está pasadas.toml y con qué orden se termina. Lanzar esa orden (--banco-comprobar <tanda>): no carga el modelo, y las notas que deja son las mismas que habría dejado la tanda completa. Pasó dos veces el 12/11/2026, cuando las pruebas iban todavía a una máquina aparte; las dos se terminaron así, sin repetir los 42 minutos de tarjeta.

Paso 2: el equipo queda como estaba. Al terminar, comprobacion\ ya no existe dentro de la tanda, y el informe.md dice si quedó algo fuera de ella.

Paso 3: las notas de oficio son las de una persona. Escoger dos encargos de oficio y una semilla. Copiar a mano su carpeta trabajo\<id>-semilla-<s>-pruebas a una carpeta aparte (y borrarla después), compilar y pasar las pruebas como lo haría una persona, y comparar con comprobaciones_superadas y comprobaciones_total del resultados.toml. Tienen que coincidir.

Paso 4: las rúbricas. Abrir los ficheros de rubricas\, puntuarlos según la rúbrica de cada uno y rehacer el informe:

portable\programator.exe --banco-informe evaluacion\banco\resultados\<tanda>\resultados.toml

El aviso de rúbricas pendientes tiene que desaparecer del informe.md y aparecer la media de criterio.

Paso 5: repetible. Fijar capas_gpu en banco.toml y pasar una segunda tanda con la misma orden y la misma configuración. Después:

portable\programator.exe --banco-comparar evaluacion\banco\resultados\<tanda-1> evaluacion\banco\resultados\<tanda-2>

Tiene que decir «✅ Las dos tandas son repetición una de otra», con todas las pasadas idénticas. Si dice que no son comparables, el motivo va primero (otras capas, otra versión…). Si alguna pasada se separa, nombra la vuelta: se abren las dos conversaciones de esa pasada en trabajo/<id>-semilla-<s>/.gestor/programator/ y se mira qué cambió antes de comparar modelos. La excepción declarada es una pasada cortada por el tiempo máximo, que depende del reloj y sale marcada.

Paso 6: el registro de modelos. Con todas las rúbricas de una tanda puntuadas:

portable\programator.exe --registro-anotar evaluacion\banco\resultados\<tanda>

Tiene que decir «✅ Anotado en …registro-de-modelos.toml», con el modelo, la tarjeta y las notas de las tres familias, que tienen que coincidir con las del informe.md de la tanda. Repetir la orden tiene que decir que sustituye a la entrada anterior y que ninguna cifra cambia. Con una tanda anterior al 12/11/2026 tiene que negarse, porque no guarda la tarjeta.

Después se arranca Programator sobre una carpeta de prueba y se abre .gestor/canal/COMO-ENCARGAR-A-PROGRAMATOR.md. La sección «En qué acierta y en qué no, medido» tiene que traer la tabla del registro, con la fecha y la tarjeta de la tanda, y no nombrar el proyecto privado en que se midió al principio. Con otro contexto en programator.toml que el de la tanda, tiene que llevar el aviso «Ojo».


Prueba J — la comprobación local, sin máquina aparte

Por qué existe. Las pruebas automáticas de la comprobación local usan un lanzador de mentira y no ejecutan más que un echo. Solo esta prueba ejecuta el código del modelo con bash, las herramientas de verdad y las variables que desvían las cachés, y mira si queda algo fuera.

Nunca en el equipo de uso diario. Se ejecuta código del modelo sin revisar. Se hace en una máquina Windows de pruebas, que se deja limpia al acabar.

Paso 0: lo que hay. En la máquina de pruebas: el ejecutable, evaluacion\banco\encargos\, una tanda ya pasada con su resultados.toml, pasadas.toml y trabajo\, y un banco.toml con [comprobacion_local] descomentada (de banco.ejemplo.toml) y los nombres que pida ese Windows. Anotar qué herramientas hay: bash de Git for Windows, Python, Node, .NET, Lua, SQLite y Rust.

Paso 1: sin la sección no ejecuta nada. Con [comprobacion_local] comentada, programator --banco-comprobar <tanda> termina sin crear comprobacion\, y las notas de oficio salen «no evaluado». Con una sección [comprobacion] de las de antes, se niega a arrancar y dice que se cambie por [comprobacion_local].

Paso 2: la comprobación. Con [comprobacion_local] descomentada, y sin pedir autorización (ejecutar en el equipo donde corre es para lo que nació Programator):

programator --banco-comprobar <tanda>

Tiene que decir que comprueba en este equipo y en qué carpeta, y avisar de las herramientas que no responden. Al terminar se tienen que cumplir tres cosas. La primera, que comprobacion\ ya no existe dentro de la tanda. La segunda, que el informe.md dice «en el equipo que pasó la tanda (windows)» y que fuera no quedó rastro, o lista lo que quedó. La tercera, que las notas de oficio coinciden, herramienta a herramienta, con las de la misma tanda comprobada en una máquina Linux de pruebas. Si alguna no coincide, se explica antes de dar la prueba por buena.

Paso 3: el bash de WSL se rechaza. Si la máquina tiene WSL, poner en interprete el bash.exe de System32: tiene que negarse y explicar por qué.

Hecha el 12/11/2026 en una máquina Windows de pruebas. Las 51 notas coinciden con las de Linux, sin rastro fuera. El paso 3 no se pudo hacer porque la máquina no tenía WSL. Lo que conviene saber para repetirla es lo siguiente.

Un Windows recién instalado no trae Git, Node, .NET, Lua ni SQLite. Sin el bash de Git no empieza. Sin las demás, sus encargos salen «no evaluado». Se pueden instalar con winget, y al terminar se deja la máquina como estaba.

En ese Windows hacen falta python3 = "python" y "lua5.4" = "lua" en [comprobacion_local.nombres].

Si una nota no coincide con la de Linux, se mira primero si la entrega es la misma y si algo se compiló fuera de la copia de la entrega. Así se encontró la carpeta común de cargo, que dejaba a una entrega heredar lo compilado para otra.


El aislamiento (mejoras del 12/11/2026)

Las mismas reglas: en máquinas de pruebas, nunca en el equipo de uso diario, y cada una devuelta a su estado limpio al acabar.

Paso 4: la etiqueta y la tabla funcionan. En la máquina Windows de pruebas, desde la raíz del repositorio:

cargo test --test aislamiento_windows -- --ignored

Un proceso de integridad baja no puede escribir en una carpeta normal y sí en una marcada. Y la tabla de conexiones ve la de un proceso del Job a un servidor de la prueba en 127.0.0.1.

Paso 5: aislado, las mismas notas. El paso 2 con el banco.toml de antes, que no escribe aislamiento, así que va con integridad_baja. Las 51 notas tienen que coincidir con las de Linux, sin rastro fuera, y el informe tiene que decir cómo se aisló. Si una herramienta falla por el aislamiento, se busca qué escribe y dónde, se desvía a la carpeta de la tanda con su variable, y se repite. Las dudas de partida: dotnet y el registro de usuario, npm y %APPDATA%\npm, y el estado del bash de Git.

Paso 6: escribir fuera falla, y el árbol muere entero. Con una ficha de prueba, que solo existe para esta aceptación, se comprueban dos cosas. Primero, una orden que crea un fichero en %USERPROFILE% y otro en C:\ tiene que fallar las dos veces. Después, una orden que deja un hijo colgado (sleep 600 & y luego sleep 600), con un tiempo_maximo_comprobacion_segundos corto: al vencer no queda ningún proceso de ese árbol en el Administrador de tareas.

Paso 7: un intento de red hace fallar el paso. Se preparan tres comprobaciones: una con curl https://example.com, otra que lo hace sin proxy (curl --noproxy '*' https://example.com) y otra con python -c "import urllib.request; urllib.request.urlopen('https://example.com')". Las tres cuentan como fallidas aunque salieran con 0. La terminal lo dice al terminar la semilla, con el encargo y la semilla, una vez y sin repetir destinos, y el informe añade «cuenta como fallida». En la misma tanda, E04 y E06 tienen que descargar en [preparar] sin que nada salga anotado: sus notas, como en Linux.

Paso 8: Linux, sin red. En una máquina Linux de pruebas, con [comprobacion_local]: las 51 notas, y una orden con curl que no llega a la red. Si la máquina prohíbe los espacios de nombres de usuario, el mensaje tiene que explicarlo y decir que existe aislamiento = "ninguno".


Prueba K — la revisión de modelos, autónoma, de la búsqueda al informe

Por qué existe. Las pruebas automáticas de la revisión usan un catálogo falso, un taller falso y una mesa falsa. Solo esta prueba habla con Hugging Face de verdad, baja un modelo entero, suelta el motor, pasa el banco, cambia (o no) el modelo por defecto y lo retira. Sigue plantillas/revision-mensual.md.

En el portable de uso real, y sin pedir permisos. Quien dirige abre los dos y lee el informe. Las pruebas ocultas de las tandas se ejecutan en ese mismo equipo, el único que usa Programator, sin autorización de ninguna clase.

Paso 0: lo que hay. Copiar el ejecutable nuevo al portable. En el programator.toml del portable, buzones_que_ordenan = ["claude"]; en banco/banco.toml, [comprobacion_local]. Si el programator.toml aún trae maquina_autorizada u otra clave de la máquina en [revision], Programator arranca igual y avisa de cada una. Anotar el modelo en uso y qué hay en modelos/.

Paso 1: quien dirige abre los dos. Programator sobre la carpeta del portable y el modelo de pago sobre la misma carpeta. Le pide al de pago que mire si hay modelos mejores.

Paso 2: buscar. El de pago deja «revisión: buscar». Programator, sin cargar el motor, publica cuántos candidatos hay y dónde está candidatos.md. Un «revisión: buscar» desde otro buzón se rechaza con su motivo.

Paso 3: probar. El de pago elige y pide «revisión: probar». Programator tiene que hacer cinco cosas. Primero, soltar el motor propio antes del banco: en el administrador de tareas, un solo llama-server, el del banco. Segundo, negarse con un candidato que no está en la lista, y seguir con el siguiente. Tercero, si las pruebas ocultas de un candidato fallan, dejarlo «sin comprobar» con su motivo, sin esperar ni reintentar, para que el cerrar lo vuelva a intentar. Cuarto, publicar cómo quedó cada candidato y las rúbricas que puntuar. Quinto, con un encargo normal después, arrancar otra vez el motor con el modelo por defecto.

Paso 4: puntuar y cerrar. Un «revisión: cerrar» con rúbricas sin nota no hace nada y las lista. Puntuadas, el cierre anota el registro, aplica la regla y lo publica. Si el modelo por defecto no gana nada y hay otro que lo sustituya, programator.toml cambia «[motor] modelo» y conserva los comentarios, el ciclo lo dice en la terminal, y el siguiente encargo arranca el nuevo.

Paso 5: la retirada. Para verla sin tocar los modelos de verdad: un .gguf falso dentro de modelos/, con una entrada de registro que no gane nada. El cierre lo anota con su motivo y lo borra.

Paso 6: el informe. El de pago deja revisiones/AAAA-MM/informe.md y lo resume en su buzón. Quien dirige lo lee y no ha tenido que autorizar nada.


Aviso: solo el cierre con Ctrl+C mata a llama-server

ServidorLocal (src/motor/proceso.rs) mata a llama-server en su Drop, pero Drop solo se ejecuta si el proceso termina de forma ordenada (la función main devuelve, o hay un panic! con unwind). En Windows, si se detiene un programa de consola con Ctrl+C y el programa no ha registrado su propio manejador de consola, el comportamiento por defecto del sistema es terminar el proceso en el acto sin ejecutar destructores. Por eso Programator instala al arrancar un manejador de Ctrl+C (la crate ctrlc, en src/motor/proceso.rs): al recibirlo, mata el llama-server que arrancó, espera a que termine y sale con el código 130. Ese manejador solo engancha Ctrl+C y Ctrl+Break. Cerrar la ventana de la consola, matar el proceso desde el Administrador de tareas o cerrar la sesión de Windows no pasan por él, y por esos caminos llama-server puede quedar huérfano con el modelo cargado en la VRAM. La prueba pendiente 1 de arriba debe repetirse cerrando específicamente con Ctrl+C, no solo cerrando la ventana de la consola, antes de darla por buena.
