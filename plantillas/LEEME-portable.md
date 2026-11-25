Programator — Guía de puesta en marcha

Esta carpeta contiene todo lo necesario para ejecutar Programator. La aplicación es totalmente portable y no requiere instalador: basta con copiar el directorio completo en la ubicación deseada del sistema y ejecutarlo desde allí.


Contenido de la carpeta

El fichero programator.exe es el ejecutable del arnés; en el paquete de Linux se llama programator, sin extensión. Es el único binario que se inicia directamente.

El fichero programator.ejemplo.toml es una plantilla comentada con todas las opciones disponibles y sus valores por defecto.

El fichero programator.toml es el archivo local de configuración, con las rutas y parámetros propios de tu equipo. No viene en el paquete: lo crea el script preparar-portable a partir de programator.ejemplo.toml si todavía no existe, y si ya existe no lo toca. También puedes crearlo tú copiando el ejemplo.

Los scripts preparar-portable.ps1, en el paquete de Windows, y preparar-portable.sh, en el de Linux, descargan el motor y el modelo, como se explica más abajo.

El fichero programator.ico es el icono distintivo de la aplicación. Solo viene en el paquete de Windows.

El fichero ultima-carpeta.txt es un registro automático con la última carpeta de trabajo utilizada, generado por el propio arnés. No viene en el paquete: aparece tras la primera ejecución.

Los ficheros INSTALACION.md y DEFECTOS-CONOCIDOS.md son la guía de instalación paso a paso y la lista de defectos conocidos. Las comprobaciones que requieren hardware real y no pueden automatizarse en la suite de pruebas no vienen en el paquete: están en el repositorio, en el documento docs/pruebas-manuales.md.

El fichero LICENSE es la licencia de Programator.

La carpeta plantillas/ contiene los documentos y guías que el arnés instala o publica dinámicamente en el proyecto donde trabaje.

La carpeta banco/ trae los encargos del banco de modelos, en banco/encargos/, y su configuración, banco/banco.toml, para medir un modelo en esta máquina.

La carpeta vigilante/ trae gestor-vigilante (gestor-vigilante.exe en Windows), el vigilante del canal de Gestor Agentes, con su licencia en Licencia.txt y su propio LEEME.txt. Es un programa de terceros, con licencia GPL-3.0-or-later. Programator lo copia al proyecto en el que trabaja si allí no hay ninguno, como se cuenta más abajo.

La carpeta herramientas/ es el directorio donde reside el motor, llama-server.exe en Windows y llama-server en Linux, junto a sus bibliotecas de enlace dinámico.

La carpeta modelos/ es el directorio destinado a alojar los modelos cuantizados en formato GGUF.

Las carpetas herramientas/ y modelos/ no se distribuyen con el repositorio debido a su gran volumen: el motor pesa algo más de un giga y el modelo trece. Si tu copia no las incluye, hay dos formas de conseguirlas.

La corta. Lanza el script que viene en esta misma carpeta y se ocupa de todo. En Windows la orden es la primera de estas dos, y en Linux la segunda:

    powershell -ExecutionPolicy Bypass -File preparar-portable.ps1
    ./preparar-portable.sh

Descarga el motor y el modelo de sus sitios oficiales, comprueba que lo descargado es exactamente lo que debía ser, lo deja donde el programa lo espera y termina enseñándote el diagnóstico de tu máquina. Si se corta a mitad, se relanza y continúa donde se quedó. Y si alguna descarga falla, te dice qué fichero necesitaba, de dónde, cuánto pesa y dónde dejarlo, para que puedas terminarlo a mano.

La larga, si prefieres controlar tú cada pieza, está al final de este documento, en la sección de componentes necesarios. El detalle completo, incluido qué hacer si alguna de esas direcciones deja de existir, está en el documento INSTALACION.md de esta carpeta (en el repositorio, docs/instalacion-paso-a-paso.md).


Puesta en marcha en tres pasos


Primer paso: comprobar los componentes del sistema

Antes de arrancar o cargar el modelo en memoria, puedes verificar qué recursos detecta la aplicación ejecutando la orden de diagnóstico:

programator.exe --diagnostico

Esta orden analiza el equipo e imprime un resumen con la tarjeta gráfica detectada, la memoria VRAM libre, la disponibilidad de soporte CUDA en el motor, el modelo configurado, el cálculo de capas del modelo que pueden alojarse en la GPU y la carpeta de trabajo. Debajo añade la lista de los modelos que hay en la carpeta modelos/. No abre la ventana de la carpeta ni arranca el motor. La forma es esta; lo que va entre ángulos cambia en cada equipo:

    Programator 1.1.0 — asistente de código local en segundo plano

    GPU:                <tarjeta> · <memoria libre> libres de <memoria total>
    Motor:              llama-server con CUDA, en <ruta del motor>
    Modelo:             <fichero del modelo> · <capas> capas · <tamaño>
    Encaje:             en <tarjeta> (<memoria libre> libres) caben <n> de <capas> capas (piden <memoria>); las <resto> restantes irán por CPU y el ciclo será más lento; se pedirán <n> capas
    Carpeta de trabajo: NO FIJA (elige la carpeta de trabajo en la ventana emergente)
    Modelos disponibles en <carpeta de modelos> para <tarjeta>, contando la tarjeta entera libre (el encaje de arriba cuenta la memoria libre ahora):
    - <fichero>: cabe entero; atiende <clases que le dio el banco>

La línea Encaje cambia de redacción según el caso: si el modelo entero cabe, si caben todas sus capas pero no los embeddings y la salida, si solo cabe una parte, si no cabe ni una capa, si no se pudo calcular o si usa las capas que el banco midió para ese modelo en esta tarjeta. La línea Carpeta de trabajo dice la carpeta fijada en la configuración o, si no hay ninguna, cómo se elegirá. En la lista de modelos disponibles, un modelo cuenta como que cabe si al menos tres cuartas partes de sus capas entran en la tarjeta; si no, la línea dice cuántas entran y cuántas harían falta.

Si el diagnóstico indica que falta el motor o el modelo, revisa la sección dedicada a los componentes necesarios.


Segundo paso: configurar el archivo programator.toml

El único parámetro imprescindible para arrancar es la ruta al modelo dentro de la sección [motor]. La carpeta del proyecto anfitrión en la que se ubica el canal de comunicación .gestor/canal/ puede fijarse en la clave ruta dentro de la sección [carpeta], pasarse como argumento de consola, o dejarse en blanco para elegirla a mano cada vez en la ventana que abre el programa.

Esa ventana aparece porque la clave preguntar_siempre de la sección [carpeta] viene puesta de fábrica. Si trabajas siempre en el mismo proyecto y prefieres que Programator se vaya solo a la última carpeta que usaste, pon esa clave en false y dejará de preguntar. La carpeta la recuerda en el archivo ultima-carpeta.txt, junto al ejecutable.

Todas las demás opciones cuentan con valores predeterminados adecuados para un funcionamiento equilibrado.


Tercer paso: iniciar la aplicación

Para comenzar el ciclo de trabajo, ejecuta el programa desde la terminal indicando la ruta del proyecto si no la fijaste en el archivo de configuración:

    programator.exe
    programator.exe --ruta "C:\ruta\a\tu\proyecto"

Al iniciarse, Programator entra en un bucle continuo de sondeo: examina periódicamente el canal, atiende los encargos dirigidos a él y publica sus entregas en su propio buzón. El servidor de inferencia, llama-server, no se lanza al arrancar: se lanza la primera vez que hay un encargo que atender, con el modelo que pide ese encargo. Para detener la ejecución de forma ordenada, pulsa Ctrl+C en la consola. Al hacerlo, el proceso secundario del servidor de inferencia, si Programator llegó a lanzarlo, se cerrará automáticamente junto con el arnés.

Programator determina la carpeta de trabajo evaluando cuatro fuentes por orden de prioridad: primero el parámetro --ruta en la línea de órdenes; en segundo lugar la clave ruta en programator.toml; en tercer lugar la ruta guardada en ultima-carpeta.txt si la carpeta todavía existe en el disco; y finalmente, si ninguna de las anteriores está disponible, muestra una ventana de diálogo para seleccionarla manualmente. La tercera fuente solo cuenta si has puesto preguntar_siempre = false en la sección [carpeta]: con el valor de fábrica, true, ultima-carpeta.txt no se usa y la ventana aparece en cada arranque en el que no haya --ruta ni ruta en la configuración. Así que, para que el programa pueda reiniciarse sin que nadie tenga que elegir la carpeta en pantalla, fija la carpeta con --ruta o con la clave ruta, o pon preguntar_siempre = false para que use la última que elegiste.


Tres formas de hablar con Programator

Programator lee los encargos del canal del proyecto, la carpeta .gestor/canal, y contesta en su buzón, programator.md. Lo que cambia de una forma a otra es quién escribe esos encargos.

Con un gestor de agentes, que es la recomendada. Abres desde el gestor Programator y el agente que dirige, por ejemplo Claude, sobre el mismo proyecto, y le dices al agente que lea .gestor/canal/COMO-ENCARGAR-A-PROGRAMATOR.md antes de mandarle nada. El gestor deja preparados el canal y la identidad de cada agente, y su vigilante avisa al agente en cuanto Programator responde. A cambio, hay que instalar el gestor y decirle la primera vez dónde está programator.exe.

Con un agente de pago, sin gestor. Arrancas Programator con --ruta sobre el proyecto y abres el agente en otra terminal, en la misma carpeta. Le dices que lea la guía, que escriba sus encargos en un buzón propio dentro de .gestor/canal cuya primera línea diga quién es, como # Bandeja de Claude 1, y que las respuestas llegan a programator.md. No hace falta nada más, pero la preparación la haces tú, y el agente solo se entera de las respuestas si lanza el vigilante que Programator deja en .gestor/vigilante o si mira el buzón de vez en cuando.

Escribiéndole tú, sin ningún agente, que no es lo recomendable. Creas tú un buzón en .gestor/canal, por ejemplo usuario.md, que empiece por # Bandeja de Usuario 1, y añades cada encargo al final, en una intervención nueva: una línea LATIDO: con la fecha y la hora, una línea en blanco y una línea que empiece por Para Programator: con lo que pides. No edites lo que ya estaba escrito. No gastas nada en un agente de pago, pero nadie revisa lo que entrega: la línea de la comprobación dice si una propuesta compila, no si hace lo que pediste, y el modelo local se equivoca. Revísalo tú todo antes de usarlo.


Avisos de inicio en la terminal

Para evitar confusiones sobre cuándo se puede empezar a enviar trabajo, Programator imprime en la consola un mensaje claro sobre su disponibilidad operativa.

Da igual abrir antes el agente o Programator. Si Programator nunca ha respondido en el canal del proyecto, atiende todo encargo dirigido a él, aunque se publicara antes de arrancar. Si ya respondió alguna vez, al arrancar por primera vez sin su registro de lectura toma una instantánea del canal y no vuelve a atender lo anterior. Al arrancar mira el canal enseguida, sin esperar al primer intervalo, anuncia que ya está escuchando e indica el intervalo de sondeo configurado.

En los arranques posteriores sobre proyectos ya conocidos, Programator detecta que ya existe un registro previo de lectura y avisa de inmediato de que está listo y escuchando, indicando la cadencia con la que revisará el buzón.

Al publicar la guía de trabajo en .gestor/canal/COMO-ENCARGAR-A-PROGRAMATOR.md, Programator muestra una indicación clara para el usuario, detallando la petición exacta que debe formularle a su modelo de pago habitual (Claude, ChatGPT, etc.) para que este acceda a la guía, conozca las capacidades de Programator y comience a coordinarse con él.


Novedades de la versión 1.1.0

Los modelos que faltan se reponen. El registro guarda ahora de qué repositorio y fichero de Hugging Face salió cada modelo. Si falta en la carpeta modelos uno que, según lo medido en tu tarjeta, sigue siendo el mejor en algo, o el de programator.toml, Programator lo avisa al arrancar y explica cómo pedir la revisión. Al pedir revisión: buscar, o con programator.exe --revision-buscar, se vuelve a bajar con su nombre de siempre y se comprueba que es el mismo fichero que se midió. Los que faltan y ya no ganan nada se marcan retirados en el registro, con sus cifras. Un modelo sin fichero ya no cuenta en la propuesta de la revisión, y programator.exe --retirar-modelo retira del registro uno cuyo fichero ya no está, sin borrar nada.

El arranque se lee mejor. Lo que Programator escribe en la terminal al arrancar sale en bloques separados por una línea en blanco: cada aviso de la configuración, el del comprobador de Python, el informe, la lista de modelos, el aviso de versión, la guía y lo que hay que hacer si no hay modelos. Si falla el arranque, el consejo de qué hacer depende de qué falló, la carpeta de trabajo, el motor o el resto de la configuración, y ya no de las palabras que aparezcan en la ruta de tu proyecto.

El aviso de versión nueva ya no hace esperar. La consulta a GitHub corre mientras Programator mide la máquina y su resultado sale después de la lista de modelos. Sale por el proxy del sistema, el de las variables de entorno o, en Windows, el de las opciones de internet. Si no se puede comprobar, dice por qué, sin dejar de decir que no hace falta hacer nada. La configuración sigue en la sección [actualizaciones], con sus tres claves de siempre; un tiempo de espera de cero se rechaza avisando.

Dos ajustes nuevos en programator.toml. La clave flash_attn de la sección [motor] elige la atención rápida del motor: "on", que es lo que Programator usaba siempre y vale si no la escribes, "off" o "auto"; el fichero de ejemplo explica qué cuesta cada opción. La clave maximo_salida_al_modelo de la sección [verificacion] dice cuántos caracteres ve el modelo de lo que contesta un comprobador que rechaza su propuesta: 1200 si no la escribes, que es lo de siempre; súbela si tu modelo tiene más contexto.

Si cambias el nombre del agente con la opción nombre de la sección [agente], las propuestas van a .gestor/candidatos/ seguido de ese nombre en minúsculas, como su buzón; antes iban siempre a .gestor/candidatos/programator. Con el nombre de fábrica no cambia nada.

Los textos que lees son más limpios: ya no enseñan valores tal como los guarda el programa, las rutas salen como las escribe el sistema y con una sola cosa se dice en singular.

Las entregas dicen más. Si una propuesta no pasa la comprobación, la línea del arnés dice por qué, con la línea del error del comprobador. Si el arnés le deniega algo al modelo, como una propuesta con un nombre que la sacaría de su carpeta, la respuesta lo cuenta en un apartado propio. La línea de memoria ya no dice más capas en la tarjeta que las del modelo, y el acuse RECIBIDO lleva la hora en que se leyó el encargo.


Novedades de la versión 1.0.0

Quien encarga dice quién es. Programator solo sabe la identidad de quien le encarga algo por la primera línea de su buzón, que tiene que ser # Bandeja de seguida de su identidad, por ejemplo # Bandeja de Claude 1, la misma cabecera que pone Gestor Agentes. No la deduce del nombre del fichero. Si falta, la respuesta se publica igual, pero dice qué línea falta y no avisa a nadie; la ventana también lo avisa.

Las respuestas despiertan a quien encargó, y a nadie más. Cada respuesta empieza con una línea Para seguida de la identidad de quien encargó, dos puntos y respuesta a, con el nombre corto del encargo, por ejemplo Para Claude 1: respuesta a escribe sat.py. Si el encargo trae una etiqueta, debajo va RECIBIDO, la hora, dos puntos y la etiqueta. Cualquier otro Para que aparezca en la respuesta se escribe en minúscula para no despertar a nadie. Un encargo es para Programator si una línea lleva Para Programator, con la P mayúscula, en cualquier punto de la línea, con las mismas reglas que Gestor Agentes.

El vigilante del canal. Al arrancar en un proyecto, si en .gestor/vigilante/ no hay ningún vigilante, Programator copia allí el que trae en su carpeta vigilante/. Nunca reemplaza uno que ya exista. La guía del canal le dice al agente que lo lance tras cada encargo, para enterarse en cuanto Programator responde.

La cola de pendientes. Un encargo que no se puede atender porque el modelo no termina de cargar a tiempo, porque el motor se cierra o porque la entrega no se puede publicar pasa a .gestor/<agente>/pendientes.toml, y sigue ahí aunque cierres Programator. Cada pasada atiende primero lo que hay en la cola. El tiempo que se espera a que cargue un modelo lo fija motor.espera_de_carga_segundos, 300 de fábrica, y ciclo.intentos_por_encargo dice tras cuántos intentos fallidos, 2 de fábrica, se cierra el encargo y se responde a quien lo pidió con el motivo.

Qué modelos hay. Al arrancar, y en el diagnóstico, Programator lista los modelos de la carpeta modelos/, dice si caben en la tarjeta y qué clases de encargo atiende cada uno según el banco. Un modelo cuenta como que cabe si al menos tres cuartas partes de sus capas entran en la tarjeta. Si ninguno cabe, o la carpeta está vacía, lo dice en la terminal y, si la configuración dice quiénes pueden ordenar la revisión de modelos, lo deja escrito en su buzón dirigido a ellos, sin descargar nada.

La revisión de modelos se hace en la carpeta donde está instalado Programator, que es donde están los modelos y el banco. Quién puede pedirla por el canal se dice con identidades, en la opción identidades_que_ordenan de la sección [revision]; la antigua buzones_que_ordenan ya no vale y Programator avisa al arrancar si la encuentra. Si Programator trabaja en otro proyecto, un encargo de revisión se contesta explicando cómo hacerla en la carpeta de instalación, y no hace nada más.

Las propuestas de Python se comprueban con el Python del equipo si no has configurado otra cosa, como se explica al final de este documento, en el apartado de incidencias frecuentes.


Ficheros que escribe Programator en el proyecto de trabajo

Programator opera de manera respetuosa con el proyecto anfitrión y nunca escribe archivos en la raíz de su repositorio. Toda su actividad queda confinada en las siguientes rutas.

En estas rutas, <agente> es el nombre del agente en minúsculas, programator salvo que cambies agente.nombre en programator.toml.

El fichero .gestor/<agente>/PROGRAMATOR.md contiene instrucciones y normas específicas del proyecto anfitrión. Se genera únicamente si no existe previamente y jamás se sobrescribe en arranques posteriores. Este archivo debe ser editado por el equipo para detallar las convenciones de código. Si el proyecto ya tiene un PROGRAMATOR.md en su raíz, de una instalación anterior a la 0.9.0, se usa ese y no se crea otro. Ten en cuenta que muchos proyectos, entre ellos los que coordina Gestor Agentes, ignoran la carpeta .gestor/ en git: en ese caso este fichero tampoco entra en el repositorio, y si quieres versionarlo tendrás que añadir una excepción para él en el .gitignore.

El fichero .gestor/canal/COMO-ENCARGAR-A-PROGRAMATOR.md es la guía de uso dirigida a los demás agentes y desarrolladores que interactúan en el canal. Se actualiza en cada arranque para reflejar con exactitud las herramientas disponibles y las reglas vigentes.

El fichero .gestor/<agente>/lectura.json es el registro interno de punteros y huellas de lectura del canal. Permite recordar qué partes de los buzones ya han sido leídas para procesar exclusivamente los contenidos nuevos.

El fichero .gestor/<agente>/latido.json es el archivo de estado en tiempo real que refleja la actividad del arnés.

El fichero .gestor/<agente>/pendientes.toml es la cola de encargos que llegaron y todavía no tienen respuesta publicada.

El fichero .gestor/<agente>/encaje-historico.log va acumulando cómo se repartió el modelo entre la tarjeta y el procesador: una línea en cada arranque y, por cada encargo, la misma línea Memoria que lleva su entrega.

Los ficheros .gestor/<agente>/conversacion-<marca>.jsonl guardan la conversación de cada encargo con el modelo. Solo se escriben si pones agente.registro_de_conversacion = true; de fábrica está apagado.

Las carpetas .gestor/<agente>/verificacion-<marca>/ son donde trabaja el comprobador de cada propuesta, dentro del proyecto y no en la carpeta temporal del sistema. Se borran al terminar cada comprobación.

El fichero .gestor/canal/<agente>.md es su buzón, donde publica sus entregas; lo ya atendido pasa a .gestor/canal/historico-<agente>.md.

El fichero .gestor/canal/estado.md lo escribe cuando el modelo reserva un fichero con la herramienta de reservas; la línea de la reserva se quita sola al cerrar el encargo.

La carpeta .gestor/candidatos/programator/ guarda las propuestas de código que escribe con escribir_propuesta, para que otro agente las revise antes de llevarlas al proyecto.

La carpeta .gestor/candidatos/poda/ guarda, al cerrar un encargo, una propuesta de poda del buzón de quien lo encargó: un fichero con el buzón podado y otro con lo que se archivaría. Es solo una propuesta: el buzón del otro agente no se modifica.

La carpeta .gestor/vigilante/ recibe una copia del vigilante del canal si no había ninguno, como se explica en las novedades de la 1.0.0.


El archivo de estado en tiempo real: latido.json

Para que tanto los usuarios como otros agentes automatizados puedan conocer el estado interno de Programator sin necesidad de interpretar registros de consola, el arnés actualiza el archivo .gestor/<agente>/latido.json en cada vuelta del bucle de sondeo.

Este archivo contiene información estructurada con tres posibles estados de ejecución.

El estado reposo indica que Programator ha completado sus revisiones, no tiene tareas pendientes y se encuentra esperando el momento de la siguiente comprobación. El archivo incluye, en el campo proximo_sondeo, la hora local en la que se llevará a cabo el próximo sondeo. Todas sus horas, también la del propio latido en marca_tiempo, van en la forma año-mes-día horas:minutos:segundos, por ejemplo 2026-09-25 14:03:00, la misma que usan los latidos del canal; no llevan zona horaria.

El estado atendiendo indica que el modelo de lenguaje se encuentra procesando activamente un encargo. En este estado, el archivo detalla el identificador del encargo en curso o las primeras líneas de su descripción.

El estado error indica que ha ocurrido una incidencia que impide completar el ciclo, como un fallo reiterado de comunicación con el motor de inferencia. El archivo recoge el mensaje descriptivo del fallo.

El latido de reposo, y el de error con que termina una vuelta de sondeo, llevan además el campo modelo_cargado, que dice qué modelo tiene cargado el motor propio de Programator. Si no hay ninguno cargado, porque aún no ha llegado ningún encargo, porque el modelo se soltó o porque el motor ya no sigue vivo, el campo no aparece; tampoco aparece en el latido de atendiendo. En cuanto se publica la respuesta de un encargo, el estado vuelve a reposo.

Para saber si Programator está funcionando con normalidad, basta con consultar este fichero. Si el estado es de reposo y la hora del próximo sondeo se encuentra en el futuro inmediato, el agente está plenamente activo. Si se encuentra atendiendo, el tiempo transcurrido corresponde al cálculo del modelo en la tarjeta gráfica.


Opciones avanzadas de configuración en programator.toml

Además de las opciones básicas de rutas y modelo, el archivo programator.toml permite ajustar el comportamiento del arnés mediante parámetros específicos.

La opción motor.bytes_por_token es el número estimado de bytes por cada token de texto, cuyo valor por defecto es 4. Se utiliza para calcular de forma preventiva el espacio ocupado en la ventana de contexto antes de enviar la conversación al motor.

La opción motor.umbral_aviso_contexto es el porcentaje de la ventana de contexto a partir del cual el arnés emite una advertencia preventiva, establecido por defecto en el 70 por ciento. Permite anticipar un posible desbordamiento de memoria antes de que el motor rechace la petición.

Sobre los históricos del canal: los archivos que empiezan por historico- o histórico- no son buzones de nadie, y lo archivado en ellos no se vuelve a atender. Son los del protocolo del canal y no se pueden cambiar; la antigua clave agente.prefijos_ignorados ya no vale y, si sigue en tu programator.toml, Programator lo avisa al arrancar.

La opción agente.registro_de_modelos es la ruta al registro de modelos, con lo que el banco midió de cada modelo; la guía de agentes saca de ahí su tabla de lo que el modelo hace bien y mal. Lo escribe la orden programator --registro-anotar <tanda>. Por defecto es registro-de-modelos.toml, junto a programator.toml.

La opción ciclo.max_reintentos_motor es la cantidad máxima de reintentos ante un fallo transitorio de respuesta del motor cuando la comprobación de salud confirma que el proceso sigue vivo, fijada por defecto en 2 reintentos.

La opción ciclo.espera_reintento_motor_segundos es el tiempo de espera en segundos entre reintentos sucesivos ante sobrecarga de la tarjeta gráfica, por defecto 5 segundos.

La opción ciclo.max_denegaciones_seguidas es el número máximo de insistencias consecutivas permitidas al modelo en una herramienta denegada antes de abortar el encargo, establecido por defecto en 2 (regla de los dos intentos).

La opción ciclo.tope_lectura_bytes es el límite máximo de bytes entregados al modelo en una sola operación de lectura de archivo, establecido por defecto en 16.384 bytes para proteger la ventana de contexto.

La opción motor.tiempo_lectura_segundos es el plazo máximo de espera en segundos para recibir la respuesta del motor, fijado por defecto en 600 segundos (diez minutos).

La opción motor.tiempo_escritura_segundos es el plazo máximo de espera en segundos para transmitir la solicitud al motor, por defecto 60 segundos.


Qué modelo se carga y cuánta memoria usa

Si tienes varios modelos medidos con el banco, cada encargo puede ir al que mejor hace su clase de trabajo. El agente que dirige lo pide con una línea Clase: en el encargo, y Programator carga el modelo que más nota saca en esa clase entre los que el banco midió en esta tarjeta y tienes en modelos/. Sin esa línea, lo atiende el modelo de motor.modelo. La guía COMO-ENCARGAR-A-PROGRAMATOR.md que Programator deja en el canal explica cómo se usa y trae la tabla de clases.

Tres opciones de la sección del motor deciden la memoria, y cada una está explicada con sus consecuencias en programator.ejemplo.toml. La opción motor.capas_gpu, con auto, usa las capas que el banco midió para ese modelo en esta tarjeta si caben, y si no, calcula las que caben; con un número, pone esas. La opción motor.vram_maxima_mib pone un tope de memoria de la tarjeta, en megas, que manda incluso sobre un número de capas escrito a mano. La opción motor.soltar_modelo decide cuándo se descarga el modelo: con auto, tras quince minutos sin encargos; con 0, al terminar cada encargo; con otro número, tras esos minutos.

Cada entrega lleva una línea que empieza por Memoria, con el modelo, las capas que cupieron en la tarjeta y la memoria libre antes y después. Si cupieron menos capas que las medidas por el banco, la entrega lo avisa: el encargo se hizo igual, más despacio.


El banco de modelos: tus lenguajes y las pruebas ocultas

La orden programator --banco <carpeta> mide un modelo local con una colección de encargos. Puedes añadir los lenguajes que te interesen, por ejemplo PHP, sin tocar Programator: se declaran en el banco.toml de esa carpeta y se escriben sus encargos. Lo explica, con un ejemplo completo, el documento docs/Como adaptar el banco de modelos.md del repositorio, que no viene en este paquete.

Para ver si el código del modelo funciona, el banco lo ejecuta en este equipo: son las pruebas ocultas, encendidas por defecto, y necesitan los lenguajes de los encargos instalados. Si no lo quieres, apágalas en banco.toml:

    [comprobacion_local]
    activa = false

El modelo escribe igual y su código queda guardado. Lo que había que ejecutar no recibe nota de las pruebas ocultas: si el encargo trae una rúbrica, un fichero rubrica.md en su carpeta, el banco deja el código entregado junto con la rúbrica en la subcarpeta rubricas/ de la tanda, para que quien dirige lo valore leyéndolo; solo si no la trae sale «no evaluado». Lo mismo pasa con las pruebas encendidas cuando, por cualquier otro motivo, no llegan a dar nota.


Componentes necesarios si la instalación viene sin binarios pesados

Si tu distribución no incluye las dependencias de inferencia, es necesario añadir los siguientes elementos en sus directorios correspondientes.

Primero, el motor de inferencia en la carpeta herramientas/. Requiere una compilación de llama.cpp con soporte CUDA para tu sistema. La versión verificada en este entorno es la compilación b10993, que en Windows va con CUDA 12.4 y en Linux con CUDA 12.8. El directorio debe contener llama-server.exe (llama-server en Linux) junto con sus bibliotecas auxiliares, en particular ggml-cuda.dll (libggml-cuda.so en Linux).

Segundo, el modelo de lenguaje en la carpeta modelos/. El modelo de referencia verificado en la especificación es devstral-small-2-24b-Q4_K_M.gguf.

Programator, el programa, no descarga nada por su cuenta. Las versiones de las herramientas de inferencia cambian casi a diario y cambiar el modelo cambia directamente lo que entrega, así que quién pone qué en la máquina es una decisión de quien la usa, no del arnés.

El script preparar-portable es otra cosa: lo lanzas tú, a propósito, y trae versiones fijas y comprobadas una por una. No se ejecuta solo ni se ejecuta al arrancar el programa.


Comprobaciones recomendadas

El documento docs/pruebas-manuales.md del repositorio describe verificaciones que precisan la tarjeta gráfica encendida. Entre ellas destacan tres.

La prueba F consiste en comprobar que el motor arranca configurado con una sola secuencia concurrente (n_slots = 1) y que la velocidad de generación alcanza al menos 18 tokens por segundo. Si la velocidad cae sensiblemente, parte de las capas del modelo podrían haberse asignado indebidamente a la memoria RAM del procesador central.

La prueba G consiste en comprobar que tras el arranque se procesa de forma autónoma el primer encargo pendiente sin necesidad de editar manualmente los buzones del canal.

La prueba L, nueva en la 1.0.0, consiste en comprobar con dos modelos medidos por el banco que cada encargo va al modelo de su clase, que cada modelo se carga como mucho una vez y nunca hay dos llama-server a la vez, que cada entrega lleva su línea Memoria y que el modelo se suelta tras el tiempo configurado sin encargos.


Diagnóstico y resolución de incidencias frecuentes

Si la aplicación encuentra dificultades durante la ejecución, revisa las siguientes causas habituales.

Si el mensaje indica que un argumento no es reconocido, comprueba la sintaxis de las opciones introducidas en la consola consultando programator.exe --ayuda.

Si el informe inicial muestra la palabra FALTA en el motor o el modelo, los archivos requeridos no están en herramientas/ o modelos/, o sus nombres no coinciden con los declarados en programator.toml.

Si la consola señala SIN CUDA (solo CPU), la versión de llama-server.exe instalada carece de la biblioteca ggml-cuda.dll (en Linux, llama-server sin libggml-cuda.so). Sin aceleración por hardware, la inferencia se ejecutará en el procesador central con un rendimiento sumamente bajo.

Si se asignan menos capas a la tarjeta gráfica de las previstas, reduce el tamaño del parámetro contexto en programator.toml. La memoria necesaria para retener la conversación compite directamente por la misma VRAM que las capas del modelo.

Si publicas un encargo en el canal y Programator no responde, asegúrate de que el encargo está en el buzón de otro participante, en una línea que lleve Para Programator, con la P mayúscula (o bajo un encabezado «## Para Programator», la forma antigua), y no con Programator solo en copia ni nombrado sin Para. Comprueba asimismo que latido.json refleja el estado de reposo y una hora de próximo sondeo cercana. El momento en que se publicó el encargo solo importa en un caso: si Programator ya había respondido alguna vez en ese canal y ha perdido su registro de lectura, lectura.json, al arrancar da por visto todo lo que ya había, y un encargo publicado antes de su aviso de disponibilidad no se atiende; vuelve a publicarlo al final del buzón. Si tu buzón no empieza por la línea # Bandeja de con tu identidad, Programator sí responde, pero la respuesta no te despertará.

Si se notifica que la carpeta de trabajo no existe, verifica la ruta asignada en el parámetro --ruta, en la clave ruta del archivo de configuración o en ultima-carpeta.txt.

Si una propuesta de código aparece calificada como SIN COMPROBAR, no se ha configurado ninguna orden de validación para esa extensión en la sección [verificacion.comprobadores]. Esto indica que el arnés no ha podido evaluar la sintaxis del archivo, por lo que su validez debe ser comprobada manualmente por el desarrollador.

Los ficheros de Python son la excepción: si no has puesto una orden para py, Programator busca al arrancar un Python que responda, primero python y luego python3, y si lo encuentra comprueba con él la sintaxis de las propuestas .py, sin ejecutarlas y sin dejar ninguna carpeta __pycache__. La terminal dice con qué Python las comprueba o, si no encontró ninguno, que quedarán sin comprobar. Si tu Python no está en el PATH, pon su ruta en la opción python de la sección [verificacion]; para no buscarlo, pon comprobar_python = false. Una orden tuya para py manda siempre sobre esta búsqueda.

Si la velocidad de generación es inferior a 12 tokens por segundo, examina el registro del motor para verificar que el parámetro n_slots tiene valor 1. Si se configuran secuencias paralelas innecesarias, la tarjeta consume memoria adicional y fuerza el traslado de capas del modelo a la CPU.
