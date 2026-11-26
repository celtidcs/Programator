Defectos conocidos y limitaciones estructurales

Este documento recoge de forma explícita los defectos conocidos, las pruebas no automatizadas y las limitaciones de diseño que siguen vigentes en Programator. Su propósito es advertir con claridad de los puntos débiles del sistema para que quien lo utilice sepa qué esperar, qué consecuencias operativas existen y cómo mitigar cada situación.


Pruebas ignoradas en la suite automatizada

La suite mecánica del proyecto contiene seis pruebas marcadas con el atributo «#[ignore]». No se ejecutan durante la verificación ordinaria con cargo test porque tocan la máquina real: las dos primeras, el hardware; y las cuatro últimas, la seguridad de Windows (tests/aislamiento_windows.rs: dos crean procesos de integridad baja, una de ellas en una carpeta donde el usuario solo tiene permiso de modificar; la tercera lee la tabla de conexiones de un Job, y la cuarta comprueba que las órdenes aisladas de preparar corren con integridad baja; se ejecutan a mano en una máquina Windows de pruebas, nunca en el equipo de uso diario, con la orden cargo test --test aislamiento_windows -- --ignored). La de la carpeta donde solo se puede modificar no reproduce de verdad ese caso si se lanza desde una sesión de administrador: los privilegios de tomar posesión y de restaurar le dan el permiso que le faltaría, y pasa aunque se quite la corrección; sin ellos, pero con el testigo aún en integridad alta, falla incluso con ella, porque la carpeta la crea un administrador. Para medirlo haría falta un usuario estándar cuya carpeta solo tenga permiso de modificar. La corrección está comprobada en uso real, en una unidad con ese permiso. Las de hardware son las dos que siguen.

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


En Linux, el motor no lleva .exe

Programator funciona en Linux con una tarjeta NVIDIA: se probó el 25/11/2026 en Ubuntu 26.04 con una GTX 1070 Ti y el llama-server oficial con CUDA 12.8, con las mismas pruebas manuales que en Windows. Hay un detalle de la configuración que se olvida con facilidad: la clave binario de la sección [motor] viene escrita para Windows y termina en .exe. En Linux hay que dejarla en herramientas/llama-server, sin extensión, o el arranque fallará diciendo que no encuentra el motor.


Limitaciones estructurales pendientes de evolución

Durante una jornada de trabajo real sobre un proyecto de verdad (23 de septiembre de 2026), se identificaron cuatro limitaciones del protocolo de comunicación. La primera, que un encargo se reconocía por dónde caía en el buzón y no por sí mismo, quedó resuelta en la 1.2.0 con el registro de encargos, sin cambiar lo que escriben los agentes. La segunda, que no había prioridades ni forma de cancelar un encargo, también quedó resuelta en la 1.2.0, con el límite que se explica más abajo. La tercera y la cuarta, que el modelo no sabía qué había entregado antes y que nadie llevaba la cuenta de en qué quedaba cada propuesta, se resolvieron en la 1.2.0 con el registro de entregas, también con un límite, que se explica al final.


Cancelar un encargo en curso solo surte efecto entre dos llamadas al modelo

Desde la 1.2.0 un encargo puede llevar una línea Prioridad: alta, normal o baja, y quien encargó algo puede retirarlo con un encargo nuevo, Para Programator: cancelar: seguido de la etiqueta, el nombre o el identificador del encargo. Si el encargo aún no se ha atendido, se cancela sin gastar el modelo. Si ya se está atendiendo, el ciclo es de un solo hilo y una respuesta del modelo puede tardar minutos: Programator no corta una respuesta a medias, sino que comprueba si hay una cancelación antes de cada llamada al modelo y, si la hay, no hace la siguiente.

Consecuencia para el usuario: entre que se escribe la cancelación y que el encargo se corta pasa, como mucho, lo que tarde la respuesta del modelo que está en marcha. Lo que el encargo ya hubiera dejado como propuestas sigue en disco y la respuesta lo enumera.

Cómo mitigarlo: no hace falta hacer nada; si la espera importa, el modelo se puede detener cerrando Programator, y el encargo queda pendiente para la siguiente vez.


Un buzón escrito a trozos lentos puede dar un encargo cortado

Programator no lee las novedades de un buzón que ha cambiado hace menos de un segundo y medio (la clave buzon_estable_tras_ms de la sección ciclo): las deja para la vuelta siguiente y atiende los demás buzones. Así no toma por encargo una línea que su dueño todavía está escribiendo. Pero si quien escribe se queda parado a media línea más tiempo que ese plazo, lo cortado se lee como un encargo nuevo y se atiende, y después se atiende también el entero, porque los dos se reconocen por su texto. Y a la inversa, un buzón que cambia sin parar, a menos de un segundo y medio cada vez, no se lee mientras siga así.

Consecuencia para el usuario: con un editor que guarda a cada pulsación, o con un agente que escribe su bandeja a trozos y con pausas, puede llegar una respuesta a un encargo a medias, seguida de la del entero.

Cómo mitigarlo: escribir cada intervención de una vez, como hacen los agentes, o subir buzon_estable_tras_ms en programator.toml.


Un cierre brusco justo después de responder puede repetir una respuesta

Cuando Programator responde un encargo, primero publica la respuesta en su buzón y después anota en su registro de encargos que ya está atendido, las dos cosas en el mismo instante. Si el programa muere justo entre las dos escrituras, por ejemplo porque se cierra desde el Administrador de tareas o se va la luz, al volver a abrirlo el encargo sigue constando como pendiente y se atiende otra vez. El orden es deliberado: es preferible responder dos veces que perder una respuesta, porque lo primero se ve y lo segundo no. Lo que resuelve Programator sin el modelo, como una cancelación o una revisión de modelos, va en el orden contrario, porque repetirlo no aporta nada: si muere en medio, el encargo queda atendido sin respuesta.

Consecuencia para el usuario: en el peor caso, una segunda respuesta al mismo encargo, con su propuesta duplicada. Hasta la 1.2.0 la ventana era toda la vuelta del ciclo, que con varios encargos podía durar minutos; ahora son unos milisegundos.

Cómo mitigarlo: no hace falta hacer nada. Si llega a pasar, basta con ignorar la segunda respuesta y su propuesta.


Preparar una propuesta baja dependencias sin aislar

Las órdenes previas de la sección preparacion, las que bajan dependencias antes de compilar, se lanzan sin aislar y con red, porque las aisladas no la tienen. Programator no puede saber qué hace una orden: confía en que las previas solo descarguen, como cargo fetch, que no ejecuta los build.rs.

Consecuencia para el usuario: una orden previa que ejecute código del proyecto, como un npm install con scripts de instalación, lo ejecuta con red y con todos los permisos del usuario, fuera del aislamiento.

Cómo mitigarlo: poner como previas solo órdenes que descargan sin ejecutar nada (cargo fetch; npm install con --ignore-scripts) y dejar lo demás en las aisladas.


En Windows, las órdenes aisladas de preparar tienen red

En Windows, el aislamiento de las órdenes de preparar es el de las pruebas ocultas del banco: integridad baja y un Job. La integridad baja impide escribir fuera de la copia, pero no corta la red. Programator vigila las conexiones de los procesos del Job y las cuenta en la respuesta, pero no las impide.

Consecuencia para el usuario: el código de una propuesta preparada en Windows puede conectarse a la red mientras se compila o se prueba, aunque no pueda escribir en el resto del equipo.

Cómo mitigarlo: leer en la respuesta la línea de los intentos de conexión, y preparar en Linux, donde las órdenes aisladas no tienen red, lo que no sea de fiar.


Lo que el modelo dice de su propia entrega no es fiable

En la prueba de uso real de la 1.2.0, el 26/11/2026, Devstral Small 2 24B atendió nueve encargos con el modelo. Cinco entregas cumplieron todo lo medido, pero el modelo falló de cinco maneras que Programator no puede evitar, porque son de lo que escribe. Escribió en un fichero .py la lista de cumplimiento en prosa en lugar del código. Validó fechas con int(), que deja pasar signos, espacios, guiones bajos y cifras que no son ASCII, y aseguró que solo aceptaba la forma AAAA-MM-DD. En una función que recorta textos usó tres puntos en lugar del carácter de puntos suspensivos que se le pidió y devolvía textos más largos de lo pedido. En un encargo de dos partes hizo la primera y publicó sin hacer la segunda. Y en varias respuestas dijo que cumplía todos los requisitos o que lo había verificado compilando, cosa que no puede hacer.

Consecuencia para el usuario: la respuesta del modelo no basta para saber si la entrega sirve.

Cómo mitigarlo: creer las líneas que escribe el arnés, que nombran cada propuesta con su comprobación y dicen si el encargo no dejó ninguna; configurar un comprobador para la extensión; pedir los requisitos en una lista; y probar la entrega antes de aceptarla.


Programator solo deduce que se aceptó una propuesta copiada tal cual

Desde la 1.2.0 cada propuesta queda anotada en .gestor/programator/entregas.toml, con su encargo, el modelo que la escribió, su comprobación y su estado, y el modelo recibe en cada encargo la lista de lo que ya entregó y en qué quedó. El estado lo dice el agente que encargó, con Para Programator: aceptada: o rechazada: seguido del nombre de la propuesta, y si no dice nada lo deduce Programator: la da por aceptada si encuentra en el proyecto un fichero con el mismo nombre y el mismo contenido, sin contar los saltos de línea, y por sustituida si después entregó otra para el mismo nombre. Lo que pasa treinta días sin nada queda sin confirmar, y lo resuelto sale de la carpeta de candidatos a su subcarpeta archivo.

Consecuencia para el usuario: una propuesta que se aplica con cambios, o con otro nombre, no se puede deducir. Se queda como pendiente de revisión hasta que el agente lo diga o pasen los treinta días, y mientras tanto el modelo la ve como «propuesto, sin confirmar» y no como aceptada. La búsqueda de lo aceptado recorre el proyecto, y para no hacerlo en cada pasada se hace como mucho una vez cada sesenta minutos, así que una copia tarda hasta una hora en notarse.

Cómo mitigarlo: decir en qué quedó cada propuesta, como explica la línea con que termina su aviso en el canal. El intervalo de la búsqueda y el plazo de los treinta días se cambian en la sección entregas de programator.toml.


Incidencia sobre la primera pasada del canal

Conviene dejar constancia de que una incidencia observada en aquella jornada de uso real ha quedado descartada como defecto. La guía de usuario sugería erróneamente que en cada inicio del programa se ignoraban los textos preexistentes en el canal.

Hoy la instantánea inicial solo se plantea al arrancar sin registro de lectura, .gestor/<agente>/lectura.json, y solo descarta lo anterior si Programator ya respondió alguna vez en ese canal, es decir, si su propio buzón tiene alguna intervención con su LATIDO. En un proyecto donde nunca ha respondido, que es el caso de un proyecto nuevo, la instantánea no da nada por visto y Programator atiende todo encargo dirigido a él, aunque se publicara antes de arrancar; da igual abrir antes el agente o Programator. El caso en que lo anterior se descarta es, por tanto, el de un canal con respuestas previas cuyo registro de lectura se ha perdido; lo descartado queda anotado como visto en el registro de encargos, y no se atiende aunque su buzón cambie después. En cualquier arranque con registro, Programator conserva su memoria y atiende cualquier encargo que no tenga ficha, se escriba donde se escriba dentro del buzón.
