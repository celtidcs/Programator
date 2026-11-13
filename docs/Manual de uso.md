Manual de uso de Programator

Este manual está pensado para quien abre Programator por primera vez y no sabe todavía para qué sirve. No hace falta saber nada del programa para leerlo. Explica qué hace, para qué se usa y cómo se pone en marcha, paso a paso. Los detalles de instalación, con las direcciones exactas de descarga, están en el documento Instalación paso a paso, que acompaña a este.


Qué es Programator

Programator es un programa para Windows y Linux que convierte un modelo de inteligencia artificial que funciona en tu propio ordenador, sin conexión a internet y sin pagar por uso, en un ayudante más de tu equipo de trabajo. El modelo corre en tu tarjeta gráfica gracias a un motor llamado llama-server, que Programator arranca y apaga por ti.

Lo importante es cómo trabaja ese ayudante. El modelo nunca toca tus ficheros. Cuando quiere leer algo, lo pide, y Programator decide si se lo deja leer. Cuando quiere entregar código, escribe una propuesta en una carpeta aparte, y ahí se queda hasta que alguien la revise y decida qué hacer con ella. No ejecuta órdenes, no navega por internet y no aprueba nada. Por eso se puede dejar trabajando sin miedo a que estropee un proyecto.

Programator no trabaja solo. Lo dirige otro agente, normalmente uno de pago como Claude, Codex o Gemini, que es quien le encarga el trabajo, revisa lo que entrega y decide si vale. Programator es su ayudante: hace tareas acotadas con su propio modelo, en tu máquina, y así el agente de pago gasta menos y puede ocuparse de lo que de verdad necesita su criterio.


Para qué sirve: los dos usos

Programator tiene dos usos, y conviene tenerlos claros desde el principio, porque todo lo demás se entiende a partir de ellos.

El primer uso es ayudar a crear y verificar código. Abres Programator sobre un proyecto, el agente que dirige le manda encargos concretos, como escribir una función, revisar un fichero buscando números sueltos o métodos demasiado largos, o preparar un fichero nuevo con unos requisitos dados, y Programator los resuelve con el modelo local. Cada propuesta de código que entrega pasa además por una comprobación que hace el propio Programator, no el modelo, de modo que el agente que dirige sabe si compila antes de mirarla.

El segundo uso es probar modelos locales, para saber si sirven para el primer uso. Cada pocas semanas aparecen modelos nuevos, y no todos valen para lo mismo: uno puede escribir buenas funciones y fallar al revisar código, y otro al revés. Programator trae un banco de pruebas, que es una colección de encargos con su solución conocida, y lo pasa entero a un modelo. El resultado es una nota por cada clase de trabajo, la velocidad y la memoria que necesita. Con esas cifras, el agente de pago decide si el modelo es adecuado. Programator mide, pero el juicio de si un modelo vale lo da quien lo dirige.

Los dos usos comparten una exigencia: que todo sea verificable y predecible. La misma prueba, repetida, tiene que dar el mismo resultado, aunque ese resultado no sea el que esperábamos. Una herramienta que contesta una cosa distinta cada vez que se le pregunta lo mismo no sirve para trabajar ni para medir. Más abajo, en el apartado sobre resultados repetibles, se explica cómo lo consigue Programator.


Qué necesitas en el ordenador

Una tarjeta gráfica de NVIDIA con memoria suficiente para el modelo. El modelo de referencia, Devstral Small 2 de 24 mil millones de parámetros, ocupa unos trece gigas, y va bien en una tarjeta de dieciséis. Con menos memoria también funciona, pero parte del modelo pasa al procesador y el trabajo se hace más lento.

No hay que instalar Programator. Es una carpeta que se copia donde quieras y se usa tal cual. Dentro van el programa, su configuración, el motor y los modelos. El motor y el modelo pesan mucho, así que no siempre vienen en la carpeta. Si faltan, el guion preparar-portable, que está en la misma carpeta, los descarga de sus sitios oficiales, comprueba que lo descargado es exactamente lo que debía ser y los deja donde el programa los espera. Se lanza a propósito, una vez, y nunca se ejecuta solo.


Cómo comprobar que todo está en su sitio

Antes de nada, abre una terminal en la carpeta de Programator y escribe:

programator.exe --diagnostico

Programator mira la máquina sin arrancar nada y te dice qué tarjeta gráfica ha encontrado y cuánta memoria tiene libre, si el motor está en su sitio y tiene soporte para la tarjeta, qué modelo va a usar y cuántas de sus capas caben en la tarjeta. Si algo falta, lo dice con su nombre y su ruta. En Linux la orden es la misma sin la terminación .exe.


La configuración

Todo lo que se puede ajustar está en un fichero de texto llamado programator.toml, junto al programa. Junto a él hay otro, programator.ejemplo.toml, que trae todas las opciones explicadas una por una con su valor por defecto. Lo único imprescindible es decirle dónde está el modelo. Todo lo demás tiene un valor por defecto pensado para que funcione bien sin tocarlo.

Hay un ajuste que merece explicación aparte, porque es el que más se nota: cuánta tarjeta gráfica usa Programator. El modelo está hecho de capas, cuarenta en el caso de Devstral, y cada capa que cabe en la tarjeta se calcula mucho más deprisa que una que tiene que ir al procesador. La opción se llama capas_gpu, en la sección del motor. Con el valor auto, que es el de fábrica, Programator mide la memoria libre de la tarjeta, pesa el modelo, reserva un margen de seguridad y pone en la tarjeta todas las capas que caben. Si escribes un número, por ejemplo 20, pone exactamente esas capas en la tarjeta y el resto en el procesador, sin discutir.

Tiene sentido elegir menos capas a propósito. Puede que quieras dejar media tarjeta libre para otro programa, para un juego o para otro modelo, o que prefieras que la tarjeta trabaje menos, se caliente menos y gaste menos. El precio es la velocidad, y se nota mucho. Medido con Devstral en una tarjeta de dieciséis gigas: con 39 de sus 40 capas en la tarjeta escribía unos 23 tokens por segundo, que son trozos de palabra; con 35 capas, unos 12; con 30, unos 7. Con 20 capas iría todavía más despacio. Programator no te lo impide, porque es tu máquina y tu decisión, pero el diagnóstico te dice siempre cuántas capas caben, para que decidas sabiendo lo que cuesta.

Hay otro ajuste relacionado, el tamaño del contexto, que es cuánto texto puede tener en la cabeza el modelo a la vez. Por defecto son 16384 tokens. Subirlo a 32768 deja leer encargos más largos, pero la memoria que necesita sale de la tarjeta, y caben menos capas.


Dos formas de trabajar: con un gestor de agentes o solo con un agente de pago

Programator siempre trabaja a las órdenes de otro agente, pero ese agente se puede abrir de dos maneras. Las dos funcionan igual de bien. Lo que cambia es quién prepara el proyecto y quién abre cada programa.

Con un gestor de agentes. Un gestor de agentes es una aplicación que guarda la lista de tus proyectos y los abre con un agente de un clic. Es la forma más cómoda de usar Programator, porque el gestor se encarga de casi todo. Cada vez que abres un proyecto, deja dentro las instrucciones de trabajo comunes a todos los agentes y el canal, que es la carpeta .gestor/canal donde los agentes se dejan mensajes, con un buzón para cada uno. Cada agente que abre sabe además quién es y cuál es su buzón.

Para usarlo, la primera vez hay que decirle al gestor dónde está Programator, porque no lo busca solo: la ruta completa de programator.exe. A partir de ahí, el trabajo de cada día es así. En el gestor eliges el proyecto y abres Programator, que se abre en su propia ventana sobre ese proyecto. Después abres, sobre el mismo proyecto, el agente que va a dirigir, por ejemplo Claude. Cuando Programator diga que está escuchando, le dices a Claude algo así: tienes a Programator disponible como ayudante; lee la guía .gestor/canal/COMO-ENCARGAR-A-PROGRAMATOR.md antes de mandarle nada. Claude escribirá los encargos en su propio buzón, que el gestor ya le ha preparado, y leerá las respuestas en el buzón de Programator.

Solo con un agente de pago, sin gestor de agentes. También funciona sin gestor. Basta con el agente de pago y Programator, cada uno en su ventana, abiertos sobre la misma carpeta de proyecto. Lo que el gestor hacía por ti lo haces tú en dos pasos.

El primero es arrancar Programator sobre el proyecto. Abre una terminal en la carpeta de Programator y escribe programator.exe --ruta seguido de la carpeta del proyecto entre comillas. Si no le das ninguna carpeta, usa la última en la que trabajó, y si tampoco la hay, abre una ventana para que la elijas. Si el proyecto todavía no tiene canal, Programator lo crea él mismo, dentro de .gestor/canal, y deja allí su guía.

El segundo es abrir el agente de pago, por ejemplo Claude Code o Codex, en otra terminal y situado en la carpeta del proyecto, y darle por escrito lo que el gestor le habría dado hecho: que tiene a Programator como ayudante, que lea la guía .gestor/canal/COMO-ENCARGAR-A-PROGRAMATOR.md, que escriba sus encargos en un buzón propio dentro de .gestor/canal, por ejemplo claude.md, y que las respuestas de Programator aparecen en programator.md, en la misma carpeta. Programator lee como buzón cualquier fichero de texto del canal que no sea el suyo, así que no hace falta registrar el buzón en ningún sitio: basta con que exista y con que los encargos lleven su encabezado Para Programator.

La diferencia práctica es que con un gestor de agentes todo eso ya está preparado y es igual en todos tus proyectos, mientras que sin él tienes que explicárselo al agente de pago cada vez que empiezas en un proyecto nuevo. En los dos casos Programator hace exactamente lo mismo.


El primer uso, paso a paso: Programator como ayudante en un proyecto

Sea cual sea la forma en que lo hayas abierto, una vez que Programator está en marcha sobre la carpeta del proyecto el trabajo transcurre igual.

Al arrancar, Programator pone en marcha el motor y carga el modelo, lo que lleva unos segundos, y escribe en la ventana un resumen de lo que ha encontrado. Después hace dos cosas en la carpeta del proyecto. Deja en la carpeta del canal, que es .gestor/canal, una guía llamada COMO-ENCARGAR-A-PROGRAMATOR.md, escrita para el agente que lo va a dirigir. Y anuncia en la ventana que ya está escuchando. A partir de ese aviso, y no antes, puede recibir encargos.

Ahora vas al agente que dirige y le dices que tiene a Programator disponible y que lea esa guía antes de mandarle nada, como se explica en el apartado anterior. La guía le explica todo lo necesario: cómo se escribe un encargo, qué sabe hacer bien Programator, en qué falla, con cifras medidas, y qué le va a devolver.

El agente que dirige escribe el encargo en su propio buzón del canal, como cualquier otro encargo del canal: en una línea que empieza por Para Programator, con lo que pide en esa línea y en las siguientes. Si Programator solo va en copia, o se le nombra en medio de una frase, no es un encargo. La forma antigua, un encabezado que empieza por Para Programator, también se atiende. Programator revisa el canal cada cierto tiempo, tres minutos de fábrica, que se cambian con la opción intervalo_segundos. Cuando ve el encargo nuevo, se pone a trabajar. Mientras trabaja, el fichero latido.json de su carpeta dice que está atendiendo, y cuando termina vuelve a decir que está en reposo. Las propuestas de código quedan en la carpeta .gestor/candidatos/programator del proyecto, sin tocar ningún fichero tuyo, y el aviso de entrega aparece en su buzón, que es el fichero programator.md del canal. Debajo de lo que haya escrito el modelo, Programator añade si cada propuesta pasa la comprobación. Esa línea la escribe el programa, no el modelo, y es la que hay que creer. Después, el agente que dirige revisa la propuesta y decide si la usa.

Para parar Programator, pulsa Control y C en su ventana. Apaga el motor y libera la tarjeta antes de salir.


El segundo uso, paso a paso: probar un modelo

El banco de pruebas es una carpeta con encargos de tres familias. Los de oficio piden escribir código, y cada uno trae pruebas ocultas que el modelo no ve. Los de criterio piden revisar código con defectos sembrados a propósito, y se puntúan contando cuántos encuentra y cuántos se inventa. Los de uso real reproducen las clases de trabajo que se le encargan de verdad. Cada encargo se pasa varias veces, con semillas distintas, para que una buena o mala racha no engañe.

La tanda se lanza con programator.exe --banco seguido de la carpeta del banco. Programator carga el modelo, le pasa los encargos uno detrás de otro por el mismo ciclo que en el uso normal y, al terminar, deja en una carpeta de resultados dos ficheros: resultados.toml, con todas las cifras para que las lea otro programa, e informe.md, para leerlo una persona. Con Devstral, la parte del modelo dura alrededor de tres cuartos de hora, y la de las pruebas ocultas, si se hacen, se suma después.

Las pruebas ocultas ejecutan el código que ha escrito el modelo, y lo hacen en el mismo equipo donde corre Programator: Programator no usa nunca otra máquina, solo la tarjeta y el equipo donde se ejecuta. No hace falta autorizarlo en la orden. Las pruebas ocultas son opcionales y van encendidas por defecto; para apagarlas basta con poner activa = false en la sección [comprobacion_local] de banco.toml. Apagadas, el modelo escribe el código igual y queda guardado en la tanda, no hace falta ningún lenguaje instalado, los encargos de criterio y diagnóstico se puntúan igual, y los de código se valoran leyéndolos con su rúbrica, sin ejecutarlos. Lo mismo pasa con las pruebas encendidas cuando no pueden dar nota a una entrega, por ejemplo porque falta su lenguaje en el equipo. Esa nota no es comparable con la de las pruebas, y el informe dice cuál es cuál. Un encargo de código sin rúbrica sale como no evaluado, con su motivo, nunca como un cero. Una configuración antigua que todavía traiga la sección de la máquina aparte no arranca, y el banco dice qué hacer.

Conviene saber lo que supone la comprobación local: el código que ha escrito el modelo, que nadie ha revisado, se ejecuta en ese equipo, dentro de la carpeta de la tanda y con el aislamiento que permita el sistema. El banco lo recuerda al arrancar. Hace falta bash, que en Windows es el que trae Git for Windows (el de WSL no sirve, porque ejecutaría en otro sistema), y los lenguajes que piden los encargos. De serie vienen Rust, Python, Node, .NET, Lua y SQLite. El que falte deja sus encargos como no evaluados: el código está escrito y guardado, y lo único que falta es ejecutarlo para ver si funciona. Si en ese equipo algún programa se llama distinto que en Linux, por ejemplo python en lugar de python3, se dice en la configuración y las fichas no cambian.

El banco no sabe nada de ningún lenguaje: lanza las órdenes que dice la ficha de cada encargo, y una orden que termina con 0 pasa. Por eso cada usuario puede medir el modelo en los lenguajes que le interesen, por ejemplo PHP, sin tocar Programator: se declara el lenguaje en banco.toml con la orden que mira si está instalado y se escriben sus encargos. Cómo hacerlo, paso a paso y con un ejemplo completo de PHP, está en docs/Como adaptar el banco de modelos.md.

Todo se hace dentro de la carpeta de la tanda, en una subcarpeta llamada comprobacion, y las cachés de las herramientas se desvían ahí también. Al terminar, aunque algo falle, esa subcarpeta se borra. Fuera de la carpeta de la tanda el banco no borra nada: mira si alguna herramienta dejó algo en las carpetas donde suele escribir, y si encuentra algo lo dice en la terminal y en el informe, con su ruta, para que lo revise una persona. La tanda apunta además el sistema operativo en que se comprobó, porque en otro sistema las notas de oficio pueden salir distintas.

Los encargos de criterio y de diagnóstico no se pueden puntuar a máquina del todo, y los de código tampoco cuando sus pruebas ocultas no dan nota. Para ellos el banco deja unas fichas de rúbrica, que puntúa el agente de pago con una nota del cero al cinco. Después, la orden programator.exe --banco-informe seguida del fichero resultados.toml rehace el informe con esas notas.

Cuando la tanda está entera, con todas sus rúbricas puntuadas, la orden programator.exe --registro-anotar seguida de la carpeta de la tanda la apunta en el registro de modelos. Es un fichero, registro-de-modelos.toml, que vive junto a programator.toml y guarda, por cada modelo y cada tarjeta gráfica en que se midió, la nota de cada clase de trabajo, la velocidad, la memoria, las capas y el contexto, y de qué tanda sale todo eso. Nadie escribe en él a mano: si la tanda tiene rúbricas sin puntuar, o es antigua y no guarda en qué tarjeta se pasó, la orden se niega y dice por qué. Si el modelo ya estaba apuntado en la misma tarjeta, la entrada nueva sustituye a la vieja y la orden dice qué cifras han cambiado.

Del registro sale la tabla que la guía publica en el canal para el agente que dirige, la que le dice en qué acierta el modelo y en qué no. Programator busca en el registro el modelo que tiene configurado y copia sus cifras, con la fecha, la tarjeta, las capas y el contexto con que se midieron. Si ahora está configurado con otras capas u otro contexto, la guía lo avisa, porque la velocidad será otra. Si el modelo no se ha medido nunca, la guía lo dice así, sin cifras, y recomienda tratarlo como desconocido.

Si la parte de las pruebas ocultas falla a mitad, no se pierde lo hecho con el modelo. La orden programator.exe --banco-comprobar seguida de la carpeta de la tanda termina las pruebas sin volver a cargar el modelo.


Resultados repetibles

Para que la misma prueba dé el mismo resultado, Programator cuida tres cosas, y conviene conocerlas para no romperlas sin querer.

La primera es la semilla. El modelo no escribe su respuesta de golpe: elige cada trozo de texto sorteando entre los más probables. La semilla es el número con el que empieza ese sorteo. Con la misma semilla, el sorteo sale igual y la respuesta también. Programator la lleva siempre fija, en el número 42 salvo que se cambie en la configuración. Si alguna vez quieres otro intento a propósito, cambias ese número, y así la variación solo aparece cuando alguien la pide.

La segunda es la memoria del motor. Para ir más deprisa, el motor recuerda lo último que calculó y aprovecha lo que tenga en común con la petición siguiente. Se comprobó que eso hacía que la respuesta dependiera de lo que se le había pedido antes. Por eso Programator empieza cada conversación nueva sin esa memoria y solo la aprovecha dentro de una misma conversación. Cuesta unos segundos más al empezar cada encargo, alrededor de diez con un encargo largo. Es la opción empezar_en_frio, activada de fábrica.

La tercera es el reparto de capas. Repartir el modelo de otra manera entre la tarjeta y el procesador cambia ligeramente las cuentas, y con ellas la respuesta. Con el valor auto, el reparto depende de la memoria libre en ese momento, así que, si quieres repetir un resultado de un día para otro, fija el número de capas. Para saber cuál poner, cierra los programas que vayas a tener abiertos mientras trabaja Programator, ejecuta programator.exe --diagnostico y mira la línea Encaje: dice cuántas capas caben en tu tarjeta. Escribe esa cifra en capas_gpu, en lugar de auto, y desde ese momento el reparto será siempre el mismo.

Para comprobar que dos tandas del banco son de verdad la misma, la orden programator.exe --banco-comparar seguida de las dos carpetas de tanda lo dice pasada a pasada. Primero avisa si no son comparables y por qué, por ejemplo porque se pasaron con otro modelo o con otro número de capas. Después dice si cada conversación fue idéntica y, si no lo fue, en qué momento se separaron.

Repetible no quiere decir correcto. Una semilla puede repetir el mismo error todas las veces, y por eso el banco mide cada encargo con varias semillas. Y el resultado es el mismo en la misma máquina, con el mismo modelo y la misma versión del motor. En otra tarjeta saldrá otro, igual de repetible en ella.


Lo que Programator no hace

No ejecuta órdenes ni compila por su cuenta: la comprobación de las propuestas la hace el programa con las órdenes que tú configuras, nunca el modelo. No edita ficheros del proyecto. No aprueba ni acepta trabajo de nadie. No busca en internet. Y no recuerda nada de un encargo a otro: cada encargo empieza de cero, así que lo que necesite saber tiene que estar en un fichero del proyecto o en el propio encargo.

Tampoco descarga modelos ni motores por su cuenta. Qué modelo y qué versión del motor hay en la máquina lo decide quien la usa.


Si algo no va bien

Si al arrancar dice que no entiende un argumento, la orden programator.exe --ayuda enseña todas las formas de uso. Si el resumen del arranque dice que falta el motor o el modelo, o que el motor no tiene soporte para la tarjeta, el diagnóstico dice qué fichero falta y dónde lo buscaba. Si va mucho más despacio de lo esperado, mira en el diagnóstico cuántas capas caben en la tarjeta: puede que otro programa esté ocupando memoria. Y si el agente que dirige dice que Programator no atiende sus encargos, lo primero es comprobar que el encargo se publicó después del aviso de que estaba escuchando y que lleva su propio encabezado Para Programator.
