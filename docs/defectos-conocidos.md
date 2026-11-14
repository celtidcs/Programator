Defectos conocidos y limitaciones estructurales

Este documento recoge de forma explícita los defectos conocidos, las pruebas no automatizadas y las limitaciones de diseño que siguen vigentes en Programator. Su propósito es advertir con claridad de los puntos débiles del sistema para que quien lo utilice sepa qué esperar, qué consecuencias operativas existen y cómo mitigar cada situación.


Pruebas ignoradas en la suite automatizada

La suite mecánica del proyecto contiene seis pruebas marcadas con el atributo «#[ignore]». No se ejecutan durante la verificación ordinaria con cargo test porque tocan la máquina real: las dos primeras, el hardware; la tercera, la red (src/actualizacion.rs, que pregunta a GitHub); y las tres últimas, la seguridad de Windows (tests/aislamiento_windows.rs: dos crean procesos de integridad baja, una de ellas en una carpeta donde el usuario solo tiene permiso de modificar, y la otra lee la tabla de conexiones de un Job; se ejecutan a mano en una máquina Windows de pruebas, nunca en el equipo de uso diario, con la orden cargo test --test aislamiento_windows -- --ignored). La de la carpeta donde solo se puede modificar no reproduce de verdad ese caso si se lanza desde una sesión de administrador: los privilegios de tomar posesión y de restaurar le dan el permiso que le faltaría, y pasa aunque se quite la corrección; sin ellos, pero con el testigo aún en integridad alta, falla incluso con ella, porque la carpeta la crea un administrador. Para medirlo haría falta un usuario estándar cuya carpeta solo tenga permiso de modificar. La corrección está comprobada en uso real, en una unidad con ese permiso. Las de hardware son las dos que siguen.

La primera es la detección de tarjeta gráfica, en src/motor/hardware.rs. La prueba imprime_lo_que_ve_describir_gpu_en_esta_maquina imprime lo que Programator ve de la tarjeta, su nombre y su memoria VRAM total y libre, preguntando a nvidia-smi y, en Windows, si no contesta, al sistema gráfico. No comprueba nada por sí sola: lo que imprime se compara a mano con lo que se sabe de la tarjeta, y por eso solo tiene sentido en una máquina con tarjeta gráfica, no en los entornos de integración continua, que no tienen ninguna.

La segunda es la comprobación del modelo real, en src/motor/gguf/mod.rs. La prueba el_peso_del_devstral_real_cuadra_con_el_tamano_del_fichero valida la coherencia de los metadatos y tensores frente al archivo GGUF de 13,3 GiB. Requiere que el modelo completo esté descargado y sea accesible en el disco.

Consecuencia para el usuario: la junta de comunicación entre el arnés y el motor de inferencia real queda fuera de la verificación automática en frío. Una modificación inadvertida en los parámetros de arranque del motor podría compilar limpiamente y superar todas las pruebas automáticas, pero fallar en el momento de encender la tarjeta gráfica.

Cómo mitigarlo: antes de dar por buena una entrega para producción, es obligatorio ejecutar manualmente las pruebas ignoradas y seguir el protocolo de comprobación física descrito en el documento docs/pruebas-manuales.md, que está en el repositorio y no viene en el paquete portable.


En Windows, el código del modelo puede conectarse a internet al comprobarlo en local

Con [comprobacion_local] en Windows, el banco ejecuta el código que entrega el modelo en el propio equipo, con integridad baja y dentro de un Job. Eso impide que escriba fuera de la carpeta de la tanda y que deje procesos vivos, pero no corta la red: en Windows, cortarla sin ser administrador solo se consigue con AppContainer, que se descartó.

Lo que hay en su lugar es un detector, que no impide la conexión, pero la castiga. Las variables cortan las descargas de cargo, pip y npm y mandan el proxy a una trampa que anota el destino. Además, cada vigilar_red_cada_ms se miran las conexiones de los procesos de la orden. Lo que se ve hace fallar ese paso, y se dice en la terminal y en el informe.

Este detector tiene cuatro límites. El primero es que un programa hecho a propósito para saltárselo puede hacerlo. El segundo, que una conexión más breve que el intervalo puede no verse. El tercero, que las consultas DNS no se ven, porque las hace un servicio de Windows y no el proceso.

El cuarto es que las herramientas lanzan a veces programas propios que se conectan fuera. Si el programa no está en programas_del_sistema, una entrega buena puede salir como fallida. El informe dice el destino y el programa, así que se puede reconocer y añadir a la lista. Si algún día aparece una conexión que Windows abre desde dentro del proceso de la orden, sin programa propio, esta lista no la cubre: hubo una lista de servidores para eso, red_del_sistema, y se quitó el 13/11/2026 porque ningún caso medido la necesitó.

Cada intento dice qué programa abrió la conexión, como «2.20.71.150:443 (vctip.exe)», y los de programas_del_sistema no cuentan. Así se encontró el único caso medido: en una máquina Windows de pruebas (12/11/2026), E03 semilla 43 perdió una nota buena en cuatro de nueve tandas por vctip.exe, la telemetría que lanza el enlazador de MSVC, que rustc usa en Windows; va en la lista por defecto. Un programa del modelo que se llame igual que uno de la lista tampoco cuenta.

Las pruebas del banco son ejercicios fijos que no necesitan la red: lo que descargan las fichas va en [preparar], que no se vigila, y un encargo que necesite conectarse lo hace contra un servidor de sus pruebas en 127.0.0.1. Así que cualquier intento del código del modelo es raro y merece mirarse. En Linux, con espacio_de_nombres, la red sí se corta. En Windows no hay corte: lo que hay es la detección, y Programator no lleva las pruebas a otra máquina (desde el 13/11/2026 solo usa el equipo donde corre).


En Linux, un intento de conexión no se anota

Con [comprobacion_local] y espacio_de_nombres, el código del modelo corre en un espacio de red propio, sin más interfaz que su 127.0.0.1. La conexión no llega a ningún sitio, y por eso tampoco llega a la trampa, que escucha en el 127.0.0.1 del equipo, ni hay tabla que mirar. El paso no se marca como intento de red: la terminal no lo dice, y el informe no añade «cuenta como fallida».

Lo normal es que la orden falle sola, porque el programa no encuentra la red. Entonces el motivo es la salida de la propia orden, que suele decirlo («Network is unreachable», «Could not resolve host»). Pero si el código trata el error y sigue, el paso puede pasar.

Cómo mitigarlo: las comprobaciones de un encargo no deben poder pasar con la conexión fallida. Si el encargo pide conectarse, el servidor de sus pruebas comprueba que la conexión llegó. Ese servidor sí se alcanza: unshare --net crea el 127.0.0.1 del espacio apagado, y el envoltorio de serie lo enciende antes de cada orden; si no puede encenderlo, el banco lo dice al empezar. Hoy ningún encargo lo necesita.


En Linux funciona el programa, pero no se ha probado contra una tarjeta gráfica

Programator se desarrolla en Windows. Desde la versión 0.10.0 el código también compila y pasa la norma mecánica completa en Linux, y eso no es una suposición ni una comprobación de escritorio: la suite entera se ha ejecutado dentro de un Linux de verdad, con el formato verificado, clippy sin un solo aviso y todas las pruebas en verde. La integración continua lo repite en los dos sistemas en cada cambio, así que si alguna vez dejara de cumplirse, se vería.

Lo que esa comprobación no demuestra es que funcione contra una tarjeta gráfica, porque la máquina donde se ejecuta no tiene ninguna. Nadie ha arrancado todavía Programator en un Linux con una NVIDIA delante.

En concreto, lo que queda sin probar en esa combinación son dos cosas. La primera es la medición de la tarjeta, que en los dos sistemas sale de preguntarle a nvidia-smi y descomponer su respuesta; en Windows, además, si nvidia-smi no contesta, se pregunta al sistema gráfico. El análisis de esa respuesta sí está probado, con ocho casos que cubren una tarjeta, dos tarjetas, líneas ilegibles, tarjetas que dicen no saber su memoria y cifras incoherentes; lo que no se ha probado en Linux es la llamada en sí, contra un controlador instalado. La segunda es el motor de inferencia cargando el modelo en una tarjeta bajo Linux, que es la misma junta que en Windows tampoco verifica ninguna prueba automática.

Consecuencia para quien lo use: si la medición fallara, Programator diría que no sabe cuánta memoria hay, que es la respuesta prudente y no un fallo. A partir de ahí no podría decidir cuántas capas del modelo caben en la tarjeta.

Cómo sortearlo mientras tanto: fijar el número de capas a mano en lugar de dejarlo en automático, cambiando la clave capas_gpu de la sección [motor] de "auto" a un número. Conviene empezar bajo e ir subiendo hasta que el motor se queje por falta de memoria. La siguiente orden, escrita en la terminal, enseña exactamente lo que Programator intenta leer:

nvidia-smi --query-gpu=name,memory.total,memory.free --format=csv,noheader,nounits

Si ahí sale algo razonable y el programa sigue diciendo que no sabe, es un defecto y conviene contarlo.

Hay un detalle de la configuración que se olvida con facilidad: la clave binario de la sección [motor] viene escrita para Windows y termina en .exe. En Linux hay que dejarla en herramientas/llama-server, sin extensión, o el arranque fallará diciendo que no encuentra el motor.


Mensajes que confunden

Al arrancar sin versión publicada más nueva, dice «Programator 0.12.0 ya es la versión más reciente publicada en GitHub» también cuando esa versión aún no está publicada, como pasa con una compilada desde el repositorio antes de su publicación: lo que quiere decir es que en GitHub no hay ninguna más nueva.

No cambia lo que hace Programator; está pendiente de corregir.


Limitaciones estructurales pendientes de evolución

Durante una jornada de trabajo real sobre un proyecto de verdad (23 de septiembre de 2026), se identificaron cuatro limitaciones del protocolo de comunicación que actualmente permanecen pospuestas para no alterar de forma abrupta el contrato con los agentes coordinadores.


Limitación 1: detección de encargos por marcas en archivo continuo en lugar de archivos individuales

Actualmente, los encargos se redactan dentro del archivo de buzón de cada agente, en una línea que empieza por Para Programator o, en la forma antigua que se sigue atendiendo, bajo un encabezado «## Para Programator». El arnés rastrea los avances de lectura midiendo el desplazamiento en bytes dentro del archivo, junto con una huella de lo ya leído.

Consecuencia para el usuario: si un usuario o agente edita el contenido anterior de un buzón en lugar de añadir una sección al final, o si la herramienta de edición altera los saltos de línea del archivo, la huella deja de cuadrar y Programator relee el buzón entero, y vuelve a atender los encargos que sigan escritos en él aunque ya los hubiera atendido. Y si lo nuevo se añade dentro de una sección ya leída, sin su propia línea Para Programator, se pasa por alto.

Cómo mitigarlo: redactar siempre cada nuevo encargo al final del buzón, en una intervención nueva que empiece con su «LATIDO:» y con su línea Para Programator, sin modificar el historial previo.


Limitación 2: ausencia de cola formal de encargos y cancelación explícita

El arnés procesa los encargos secuencialmente en el orden en que los detecta durante el sondeo del canal. No existe un mecanismo formal para definir prioridades, cancelar un encargo que ya ha entrado en procesamiento o reorganizar la lista de tareas pendientes.

Consecuencia para el usuario: si se publica un encargo erróneo, el modelo consumirá tiempo de inferencia en la tarjeta gráfica hasta completar el ciclo o agotar el cupo de herramientas, demorando la atención de encargos posteriores.

Cómo mitigarlo: verificar cuidadosamente los requisitos y el alcance de cada encargo antes de publicarlo en el buzón.


Limitación 3: falta de catálogo persistente de entregas previas

Programator inicia cada ciclo de inferencia sin memoria de las sesiones anteriores. El arnés no mantiene un índice estructurado de las propuestas aprobadas o entregadas en días previos, por lo que el modelo no conoce las funciones o tipos que él mismo generó en encargos anteriores a menos que se le indique leer los archivos correspondientes.

Consecuencia para el usuario: el modelo puede sugerir nombres duplicados o reinventar estructuras ya entregadas si el encargo no le instruye explícitamente a inspeccionar el código preexistente mediante la herramienta leer_fichero.

Cómo mitigarlo: incluir en las instrucciones del encargo la indicación de comprobar los archivos relevantes del proyecto antes de escribir código nuevo.


Limitación 4: seguimiento de estado por propuesta ausente

Las propuestas generadas por el modelo se depositan en el directorio .gestor/candidatos/programator/ como archivos aislados. El sistema no dispone de un registro que clasifique automáticamente si una propuesta fue finalmente aceptada, rechazada o sustituida por una versión posterior.

Consecuencia para el usuario: el directorio de candidatos acumula propuestas a lo largo de las sesiones, obligando al equipo humano o al agente coordinador a realizar un seguimiento manual de cuáles han sido integradas en el proyecto.

Cómo mitigarlo: limpiar periódicamente el directorio de candidatos o archivar las propuestas que hayan sido definitivamente descartadas tras la verificación.


Incidencia sobre la primera pasada del canal

Conviene dejar constancia de que una incidencia observada en aquella jornada de uso real ha quedado descartada como defecto. La guía de usuario sugería erróneamente que en cada inicio del programa se ignoraban los textos preexistentes en el canal.

La inspección del código confirmó que dicha instantánea inicial se ejecuta únicamente la primera vez que Programator se conecta a un proyecto nuevo, cuando el archivo de registro aún no existe. En cualquier arranque sucesivo, Programator conserva su memoria de lectura y atiende cualquier encargo publicado con posterioridad a la última revisión, garantizando un comportamiento predecible.
