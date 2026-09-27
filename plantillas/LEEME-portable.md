# Programator — Guía de puesta en marcha

Esta carpeta contiene todo lo necesario para ejecutar Programator. La aplicación es totalmente portable y no requiere instalador: basta con copiar el directorio completo en la ubicación deseada del sistema y ejecutarlo desde allí.

## Contenido de la carpeta

- `programator.exe`: El ejecutable del arnés. Es el único binario que se inicia directamente.
- `programator.toml`: Archivo local de configuración con las rutas y parámetros propios de tu equipo.
- `programator.ejemplo.toml`: Plantilla comentada con todas las opciones disponibles y sus valores por defecto.
- `programator.ico`: Icono distintivo de la aplicación.
- `ultima-carpeta.txt`: Registro automático con la última carpeta de trabajo utilizada, generado por el propio arnés.
- `pruebas-manuales.md`: Comprobaciones que requieren hardware real y no pueden automatizarse en la suite de pruebas.
- `plantillas/`: Documentos y guías que el arnés instala o publica dinámicamente en el proyecto donde trabaje.
- `herramientas/`: Directorio donde reside `llama-server.exe` junto a sus bibliotecas de enlace dinámico.
- `modelos/`: Directorio destinado a alojar los modelos cuantizados en formato GGUF.

Las carpetas `herramientas/` y `modelos/` no se distribuyen con el repositorio debido a su gran volumen: el motor pesa algo más de un giga y el modelo trece. Si tu copia no las incluye, hay dos formas de conseguirlas.

**La corta.** Lanza el script que viene en esta misma carpeta y se ocupa de todo:

```
powershell -ExecutionPolicy Bypass -File preparar-portable.ps1     (en Windows)
./preparar-portable.sh                                             (en Linux)
```

Descarga el motor y el modelo de sus sitios oficiales, comprueba que lo descargado es exactamente lo que debía ser, lo deja donde el programa lo espera y termina enseñándote el diagnóstico de tu máquina. Si se corta a mitad, se relanza y continúa donde se quedó. Y si alguna descarga falla, te dice qué fichero necesitaba, de dónde, cuánto pesa y dónde dejarlo, para que puedas terminarlo a mano.

**La larga**, si prefieres controlar tú cada pieza, está al final de este documento, en la sección de componentes necesarios. El detalle completo, incluido qué hacer si alguna de esas direcciones deja de existir, está en `docs/instalacion-paso-a-paso.md`.

## Puesta en marcha en tres pasos

### 1. Comprobar los componentes del sistema

Antes de arrancar o cargar el modelo en memoria, puedes verificar qué recursos detecta la aplicación ejecutando la orden de diagnóstico:

```
programator.exe --diagnostico
```

Esta orden analiza el equipo e imprime un resumen con la tarjeta gráfica detectada, la memoria VRAM libre, la disponibilidad de soporte CUDA en el motor y el cálculo de capas del modelo que pueden alojarse en la GPU:

```
GPU:      NVIDIA GeForce RTX 4070 Ti SUPER · 14,9 GiB libres de 15,7 GiB
Motor:    llama-server.exe con CUDA, en herramientas/llama-server.exe
Modelo:   devstral-small-2-24b-Q4_K_M.gguf · 40 capas · 13,3 GiB
Encaje:   caben 39 de 40 capas en la GPU
```

Si el diagnóstico indica que falta el motor o el modelo, revisa la sección dedicada a los componentes necesarios.

### 2. Configurar el archivo programator.toml

El único parámetro imprescindible para arrancar es la ruta al modelo dentro de la sección `[motor]`. La carpeta del proyecto anfitrión en la que se ubica el canal de comunicación `.gestor/canal/` puede fijarse en la clave `ruta` dentro de la sección `[carpeta]`, pasarse como argumento de consola, o dejarse en blanco para elegirla a mano cada vez en la ventana que abre el programa.

Esa ventana aparece porque la clave `preguntar_siempre` de la sección `[carpeta]` viene puesta de fábrica. Si trabajas siempre en el mismo proyecto y prefieres que Programator se vaya solo a la última carpeta que usaste, pon esa clave en `false` y dejará de preguntar. La carpeta la recuerda en el archivo `ultima-carpeta.txt`, junto al ejecutable.

Todas las demás opciones cuentan con valores predeterminados adecuados para un funcionamiento equilibrado.

### 3. Iniciar la aplicación

Para comenzar el ciclo de trabajo, ejecuta el programa desde la terminal indicando la ruta del proyecto si no la fijaste en el archivo de configuración:

```
programator.exe
programator.exe --ruta "C:\ruta\a\tu\proyecto"
```

Al iniciarse, Programator entra en un bucle continuo de sondeo: examina periódicamente el canal, atiende los encargos dirigidos a él y publica sus entregas en su propio buzón. Para detener la ejecución de forma ordenada, pulsa Ctrl+C en la consola. Al hacerlo, el proceso secundario del servidor de inferencia se cerrará automáticamente junto con el arnés.

Programator determina la carpeta de trabajo evaluando cuatro fuentes por orden de prioridad: primero el parámetro `--ruta` en la línea de órdenes; en segundo lugar la clave `ruta` en `programator.toml`; en tercer lugar la ruta guardada en `ultima-carpeta.txt` si la carpeta todavía existe en el disco; y finalmente, si ninguna de las anteriores está disponible, muestra una ventana de diálogo para seleccionarla manualmente. Gracias a esta memoria, el programa puede reiniciarse de forma autónoma a partir de la segunda ejecución sin requerir interacción humana en la pantalla.

## Avisos de inicio en la terminal

Para evitar confusiones sobre cuándo se puede empezar a enviar trabajo, Programator imprime en la consola un mensaje claro sobre su disponibilidad operativa:

- En el primer arranque sobre un proyecto nuevo, el arnés toma una instantánea del canal para fijar la línea base sin atender encargos anteriores, anuncia que ya está escuchando e indica el intervalo de sondeo configurado en segundos. A partir de la aparición de ese aviso, cualquier nuevo encargo publicado en el canal será procesado.
- En los arranques posteriores sobre proyectos ya conocidos, Programator detecta que ya existe un registro previo de lectura y avisa de inmediato de que está listo y escuchando, indicando la cadencia con la que revisará el buzón.

## Ficheros que escribe Programator en el proyecto de trabajo

Programator opera de manera respetuosa con el proyecto anfitrión y nunca escribe archivos en la raíz de su repositorio. Toda su actividad queda confinada en las siguientes rutas:

- `.gestor/<agente>/PROGRAMATOR.md`: Instrucciones y normas específicas del proyecto anfitrión. Se genera únicamente si no existe previamente y jamás se sobrescribe en arranques posteriores. Este archivo debe ser editado por el equipo para detallar las convenciones de código y debe versionarse en el repositorio.
- `.gestor/canal/COMO-ENCARGAR-A-PROGRAMATOR.md`: Guía de uso dirigida a los demás agentes y desarrolladores que interactúan en el canal. Se actualiza en cada arranque para reflejar con exactitud las herramientas disponibles y las reglas vigentes.
- `.gestor/<agente>/lectura.json`: Registro interno de punteros y huellas de lectura del canal. Permite recordar qué partes de los buzones ya han sido leídas para procesar exclusivamente los contenidos nuevos.
- `.gestor/<agente>/latido.json`: Archivo de estado en tiempo real que refleja la actividad del arnés.

## El archivo de estado en tiempo real: latido.json

Para que tanto los usuarios como otros agentes automatizados puedan conocer el estado interno de Programator sin necesidad de interpretar registros de consola, el arnés actualiza el archivo `.gestor/<agente>/latido.json` en cada vuelta del bucle de sondeo.

Este archivo contiene información estructurada con tres posibles estados de ejecución:

- `reposo`: Indica que Programator ha completado sus revisiones, no tiene tareas pendientes y se encuentra esperando el momento de la siguiente comprobación. El archivo incluye la marca de tiempo exacta en formato ISO 8601 en la que se llevará a cabo el próximo sondeo.
- `atendiendo`: Indica que el modelo de lenguaje se encuentra procesando activamente un encargo. En este estado, el archivo detalla el identificador del encargo en curso o las primeras líneas de su descripción.
- `error`: Indica que ha ocurrido una incidencia que impide completar el ciclo, como un fallo reiterado de comunicación con el motor de inferencia. El archivo recoge el mensaje descriptivo del fallo.

Para saber si Programator está funcionando con normalidad, basta con consultar este fichero. Si el estado es de reposo y la hora del próximo sondeo se encuentra en el futuro inmediato, el agente está plenamente activo. Si se encuentra atendiendo, el tiempo transcurrido corresponde al cálculo del modelo en la tarjeta gráfica.

## Opciones avanzadas de configuración en programator.toml

Además de las opciones básicas de rutas y modelo, el archivo `programator.toml` permite ajustar el comportamiento del arnés mediante parámetros específicos:

- `motor.bytes_por_token`: Número estimado de bytes por cada token de texto, cuyo valor por defecto es 4. Se utiliza para calcular de forma preventiva el espacio ocupado en la ventana de contexto antes de enviar la conversación al motor.
- `motor.umbral_aviso_contexto`: Porcentaje de la ventana de contexto a partir del cual el arnés emite una advertencia preventiva, establecido por defecto en el 70 por ciento. Permite anticipar un posible desbordamiento de memoria antes de que el motor rechace la petición.
- `agente.prefijos_ignorados`: Lista de prefijos de archivos dentro del canal que deben descartarse como buzones de entrada, con el valor por defecto de `["historico-", "histórico-"]`. Evita que documentos archivados que contienen encargos antiguos vuelvan a ejecutarse.
- `agente.desempeno`: Ruta al documento con las cifras de rendimiento medido que se incorporan en la guía de agentes, por defecto `plantillas/desempeno-medido.md`.
- `ciclo.max_reintentos_motor`: Cantidad máxima de reintentos ante un fallo transitorio de respuesta del motor cuando la comprobación de salud confirma que el proceso sigue vivo, fijada por defecto en 2 reintentos.
- `ciclo.espera_reintento_motor_segundos`: Tiempo de espera en segundos entre reintentos sucesivos ante sobrecarga de la tarjeta gráfica, por defecto 5 segundos.
- `ciclo.max_denegaciones_seguidas`: Número máximo de insistencias consecutivas permitidas al modelo en una herramienta denegada antes de abortar el encargo, establecido por defecto en 2 (regla de los dos intentos).
- `ciclo.tope_lectura_bytes`: Límite máximo de bytes entregados al modelo en una sola operación de lectura de archivo, establecido por defecto en 16.384 bytes para proteger la ventana de contexto.
- `motor.tiempo_lectura_segundos`: Plazo máximo de espera en segundos para recibir la respuesta del motor, fijado por defecto en 600 segundos (diez minutos).
- `motor.tiempo_escritura_segundos`: Plazo máximo de espera en segundos para transmitir la solicitud al motor, por defecto 60 segundos.

## Componentes necesarios si la instalación viene sin binarios pesados

Si tu distribución no incluye las dependencias de inferencia, es necesario añadir los siguientes elementos en sus directorios correspondientes:

1. El motor de inferencia en la carpeta `herramientas/`: Requiere una compilación de `llama.cpp` para Windows con soporte CUDA. La versión verificada en este entorno es la compilación b10993 con CUDA 12.4. El directorio debe contener `llama-server.exe` junto con sus bibliotecas auxiliares, en particular `ggml-cuda.dll`.
2. El modelo de lenguaje en la carpeta `modelos/`: El modelo de referencia verificado en la especificación es `devstral-small-2-24b-Q4_K_M.gguf`.

**Programator, el programa, no descarga nada por su cuenta.** Las versiones de las herramientas de inferencia cambian casi a diario y cambiar el modelo cambia directamente lo que entrega, así que quién pone qué en la máquina es una decisión de quien la usa, no del arnés.

El script `preparar-portable` es otra cosa: lo lanzas tú, a propósito, y trae versiones fijas y comprobadas una por una. No se ejecuta solo ni se ejecuta al arrancar el programa.

## Comprobaciones recomendadas

El documento `pruebas-manuales.md` describe verificaciones que precisan la tarjeta gráfica encendida. Entre ellas destacan:

- Prueba F: Comprobar que el motor arranca configurado con una sola secuencia concurrente (`n_slots = 1`) y que la velocidad de generación alcanza al menos 18 tokens por segundo. Si la velocidad cae sensiblemente, parte de las capas del modelo podrían haberse asignado indebidamente a la memoria RAM del procesador central.
- Prueba G: Comprobar que tras el arranque se procesa de forma autónoma el primer encargo pendiente sin necesidad de editar manualmente los buzones del canal.

## Diagnóstico y resolución de incidencias frecuentes

Si la aplicación encuentra dificultades durante la ejecución, revisa las siguientes causas habituales:

- El mensaje indica que un argumento no es reconocido: Comprueba la sintaxis de las opciones introducidas en la consola consultando `programator.exe --ayuda`.
- El informe inicial muestra la palabra FALTA en el motor o el modelo: Los archivos requeridos no están en `herramientas/` o `modelos/`, o sus nombres no coinciden con los declarados en `programator.toml`.
- La consola señala SIN CUDA (solo CPU): La versión de `llama-server.exe` instalada carece de la biblioteca `ggml-cuda.dll`. Sin aceleración por hardware, la inferencia se ejecutará en el procesador central con un rendimiento sumamente bajo.
- Se asignan menos capas a la tarjeta gráfica de las previstas: Reduce el tamaño del parámetro `contexto` en `programator.toml`. La memoria necesaria para retener la conversación compite directamente por la misma VRAM que las capas del modelo.
- Publicas un encargo en el canal y Programator no responde: Asegúrate de que el encargo está redactado bajo un encabezado formal que comience por `## Para Programator` en el buzón de otro participante. Comprueba asimismo que la publicación se realizó después de que Programator mostrara su aviso de disponibilidad en la terminal o que `latido.json` refleje el estado de reposo.
- Se notifica que la carpeta de trabajo no existe: Verifica la ruta asignada en el parámetro `--ruta`, en la clave `ruta` del archivo de configuración o en `ultima-carpeta.txt`.
- Una propuesta de código aparece calificada como SIN COMPROBAR: No se ha configurado ninguna orden de validación para esa extensión en la sección `[verificacion.comprobadores]`. Esto indica que el arnés no ha podido evaluar la sintaxis del archivo, por lo que su validez debe ser comprobada manualmente por el desarrollador.
- La velocidad de generación es inferior a 12 tokens por segundo: Examina el registro del motor para verificar que el parámetro `n_slots` tiene valor 1. Si se configuran secuencias paralelas innecesarias, la tarjeta consume memoria adicional y fuerza el traslado de capas del modelo a la CPU.
