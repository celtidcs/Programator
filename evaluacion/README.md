Evaluación del candidato — Devstral Small 2 24B sobre Programator

Desde el 11/11/2026, el banco de modelos. Esta evaluación del 22/09 se hizo a mano y una sola vez. Para comparar modelos ahora se usa el banco, con la orden programator --banco evaluacion/banco, que se explica en la sección siguiente. Sus notas se calculan de otra manera y no se pueden comparar con el 2,6 de abajo.


El banco de modelos

Son quince encargos fijos (diecisiete fichas, porque E10 tiene tres variantes) que se pasan a cualquier modelo a través del ciclo real de Programator, con varias semillas, y se puntúan siempre igual. Lo que se mide es Programator entero con ese modelo, en el papel que va a hacer. Los encargos están en evaluacion/banco/encargos/, con su esquema en el README de esa carpeta.

Cómo se lanza. Desde la raíz del repositorio, con el modelo configurado en programator.toml:

programator --banco evaluacion/banco

Dónde se ejecutan las pruebas ocultas. En este equipo, el mismo donde corre Programator, que no usa nunca otra máquina. Van encendidas por defecto, aunque evaluacion/banco/banco.toml no traiga la sección [comprobacion_local], y se ejecutan dentro de resultados/<tanda>/comprobacion/. Se apagan con activa = false en esa sección: entonces el banco no ejecuta código del modelo, y lo que se puntúa con pruebas ocultas no saca nota de las pruebas (nunca 0): si el encargo trae rúbrica, se valora leyendo el código, y si no, sale «no evaluado». No se pide autorización al lanzar la tanda: lo decide la configuración. Es código del modelo que nadie ha revisado, ejecutándose aquí, y el banco lo recuerda al arrancar. Mientras se pasan los encargos, la verificación del arnés va vacía: el código solo se ejecuta en la fase de las pruebas ocultas.

Hace falta tener instalado esto.

Primero, bash. En Windows tiene que ser el de Git for Windows, que una instalación limpia no trae. Se instala con la orden winget install Git.Git o desde https://git-scm.com/download/win. El banco lo busca en su carpeta de siempre y, si no está ahí, junto al git.exe del PATH. Si está en otro sitio, su ruta va en interprete. El bash de WSL se rechaza, porque ejecutaría en otro sistema y con otras herramientas.

Segundo, las herramientas de los encargos que se quieran evaluar: Rust (cargo), Python, Node con npm, .NET, Lua y SQLite. Las que falten dejan sus encargos sin nota de las pruebas («no evaluado», o valorados leyendo el código si traen rúbrica), y el banco dice cuáles al empezar. En Windows, Python y Lua suelen llamarse python y lua, y hay que decirlo en [comprobacion_local.nombres].

Las cachés de npm, NuGet, cargo y pip, y los temporales, se redirigen a comprobacion/.cache/, y al terminar se borra comprobacion/ entera. Fuera no se borra nada: se mira si quedó algo en las carpetas donde esas herramientas escriben por defecto, y lo que haya se dice en la terminal y en el informe. La tanda guarda el sistema operativo, porque las notas de oficio en otro sistema pueden salir distintas. Todo se ajusta en banco.ejemplo.toml.

Cómo se puntúa. Depende de la clase de encargo. Los de oficio y los de función acotada se puntúan con pruebas ocultas y requisitos de texto, como fracción de comprobaciones superadas. Los de auditar formas y validaciones se puntúan comparando los hallazgos con los defectos sembrados, contando por separado los aciertos y los inventados. En los de auditar un fichero limpio solo cuentan los inventados, y cero inventados es un 5. Los de criterio y diagnóstico los puntúa el agente que dirige, con una rúbrica. También los de código que traen rúbrica, pero solo cuando sus pruebas ocultas no dan nota (apagadas, o sin poder ejecutarse): entonces quien dirige valora el código leyéndolo. Esa nota no es comparable con la de las pruebas, y los resultados dicen cuál es cuál.

Las rúbricas. Por cada encargo de criterio o diagnóstico y cada semilla, el banco deja en resultados/<tanda>/rubricas/ un fichero con la respuesta del modelo, la rúbrica del encargo y una última línea «Nota: » vacía. En un encargo de código valorado leyendo, el fichero lleva el código entregado y el motivo por el que las pruebas no dieron nota. Se rellena con un número de 0 a 5 (vale con coma) y después se rehace el informe sin volver a pasar nada:

programator --banco-informe evaluacion/banco/resultados/<tanda>/resultados.toml

Mientras quede alguna vacía, el informe lo dice arriba y no da la media de criterio.

Qué sale. En resultados/<fecha>-<modelo>/ quedan dos ficheros. Uno es resultados.toml, con todas las cifras: modelo y huella SHA-256, motor, configuración, carga, y por encargo y semilla la nota, el consumo y la velocidad. El otro es informe.md, para leer, con una tabla por clase de trabajo: las mismas filas que copia el registro de modelos con --registro-anotar y que la guía cuenta al agente de pago. La carpeta trabajo/, con el canal de cada encargo, no se versiona.

La conversación de cada pasada. El banco activa siempre registro_de_conversacion: cada pasada deja en trabajo/<id>-semilla-<s>/.gestor/programator/conversacion-<marca>.jsonl lo que el arnés le dijo al modelo en cada vuelta y lo que el modelo contestó. Sin eso, una nota baja no dice por qué (así se encontró el bucle de reescrituras del 11/11/2026).

El tiempo máximo corta. La clave tiempo_maximo_encargo_segundos no solo marca la pasada como no entregada: el ciclo deja de llamar al motor en cuanto se pasa, y el desenlace nombra lo que quedó en disco.

Repetible. La misma tanda, con la misma configuración, da las mismas conversaciones. Para eso cuentan tres cosas, medidas el 12/11/2026. La primera es la semilla, que va siempre fija. La segunda, que cada conversación empiece sin la caché de prefijos del motor ([motor] empezar_en_frio). La tercera es el reparto de capas, que conviene fijar con capas_gpu en banco.toml si se van a comparar tandas, porque con "auto" depende de la memoria libre del momento.

Si fallan las pruebas ocultas. Lo pasado con el modelo se guarda en resultados/<tanda>/pasadas.toml al acabar cada pasada. Si la fase de las pruebas ocultas falla (el intérprete, una herramienta, la copia), el banco lo dice con la orden exacta para terminar la tanda sin volver a cargar el modelo:

programator --banco-comprobar evaluacion/banco/resultados/<tanda>

Se hace con el banco.toml de ese momento: si lo que falló fue la configuración, se corrige ahí y se repite. Las tandas anteriores al 12/11/2026 no tienen pasadas.toml y no se pueden terminar así. Si al empezar las pruebas falta alguna herramienta, el banco lo avisa en el acto, y el informe dice el motivo de cada «no evaluado».

Primeras cifras. La tanda del 12/11/2026 a Devstral Small 2 24B Q4_K_M (51 pasadas, semillas 42 a 44, comprobada en una máquina Linux de pruebas) tardó 42 minutos en la tarjeta. La del 11/11/2026 tardó 4 h 45 min y no dejó notas: el arnés no le devolvía al modelo los argumentos de sus propias llamadas y este reescribía la entrega hasta el tope (ya arreglado). Las notas de oficio sin rúbrica fueron estas: E03, 2,8; E04, 3,0; E05, 4,2; E06, 0,0 (falta un using en las tres semillas); E07, 3,3; E08, 0,0 (una columna que no existe en las tres); U03, 5,0. Las de criterio y diagnóstico esperan a sus rúbricas.


La evaluación del 22/09/2026

Qué es esto. Una evaluación técnica de un modelo local como si fuera un candidato a incorporarse al equipo de desarrollo. Las órdenes se le dan a través de Programator, por el canal de ficheros, exactamente como se le darían en producción: no se le habla directamente al motor. Así se evalúan dos cosas a la vez, el candidato y la herramienta que lo integra.

Sesión del 22/09/2026. Las órdenes se dieron por el canal y las entregas se compilaron y probaron antes de puntuarlas, sin dar por bueno nada por lo que el candidato dijera de sí mismo.


Montaje

El arnés fue Programator 0.5.0, recompilado desde main con dos arreglos de esta sesión. El modelo, devstral-small-2-24b-Q4_K_M.gguf, de 40 capas y 13,3 GiB. El motor, llama-server b10993 con CUDA 12.4. La tarjeta, una NVIDIA GeForce RTX 4070 Ti SUPER de 16 GiB. En cuanto al encaje, iban de 33 a 35 capas en la GPU según la VRAM libre, y el resto por CPU. El muestreo se hizo con temperatura 0,2, semilla fija 42 y contexto 32768.

La semilla es fija a propósito: si el candidato acierta o falla, que no sea por el azar del muestreo.


Cómo se puntúa

Cada encargo se puntúa de 0 a 5, y la nota no sale de leer la respuesta por encima: el código se compila y se ejecuta, y se le pasan pruebas que el candidato no ha visto. Un código que compila y pasa sus propias pruebas es un aprobado raso; lo que separa a un junior de un senior es lo que ocurre con las pruebas adversarias.

Las notas significan esto. Un 5, que lo entregaría a producción tal cual. Un 4, correcto, con retoques menores en revisión. Un 3, que sirve, pero necesita una revisión seria antes de entrar. Un 2, que tiene la idea, pero el resultado no se sostiene. Un 1, que responde pero no resuelve. Un 0, que no entrega, o entrega algo falso.

En cada entrega se miran cinco ejes.

Primero, la corrección: ¿hace lo que se pidió? ¿Compila? ¿Pasa pruebas que no escribió él?

Segundo, el seguimiento de instrucciones: los encargos llevan requisitos explícitos y prohibiciones. Ignorar uno cuenta, aunque el código funcione.

Tercero, la robustez: entrada rara, límites, recursos. Lo que separa un juguete de un servicio.

Cuarto, la idiomaticidad: ¿escribe como se escribe en ese lenguaje, o traduce de otro?

Quinto, la honestidad: ¿dice lo que ha hecho de verdad, o adorna? ¿Admite lo que no sabe?


La batería

E01 mide el protocolo y el autoconocimiento: qué herramientas tiene y qué no puede hacer. No tiene lenguaje.

E02 mide el diseño de aplicaciones: arquitectura, conflictos, supuestos. No tiene lenguaje.

E03 mide el código de sistemas: errores tipados, sin pánico, pruebas. En Rust.

E04 mide los tipos: uniones discriminadas, validación, sin any. En TypeScript.

E05 mide el código idiomático y los casos borde. En Python.

E06 mide la asincronía y los recursos. En C#.

E07 mide las metatablas y el ámbito. En Lua.

E08 mide las consultas no triviales y los índices. En SQL.

E09 mide la depuración: encontrar un fallo ajeno. En el lenguaje que toque.

E10 mide la honestidad: qué hace ante algo que no existe. No tiene lenguaje.

SQL entra en la lista aunque no se pidiera: un candidato que escribe servicios y no sabe leer un plan de consulta es un riesgo, y en este equipo se toca base de datos.


Estructura de la carpeta

    evaluacion/
    ├── README.md                     este documento: plan y rúbrica
    └── banco/
        ├── banco.ejemplo.toml        la configuración del banco, comentada
        └── encargos/                 los encargos fijos, con su esquema

Lo que no está aquí, y por qué. El material en bruto de aquella sesión, es decir, las respuestas tal cual las escribió el candidato, los proyectos donde se compiló cada entrega y el registro de fallos del arnés que se fue tomando en caliente, se quedó en el repositorio de trabajo. Ocupa mucho, solo tiene sentido junto a la máquina en la que se midió y no aporta nada que no esté ya contado aquí con sus cifras. Tampoco se publican los informes detallados de aquella evaluación ni de las tandas del banco: lo que importa de ellos está resumido en este documento.

Se rescató una sola pieza de aquel material, banco/.gestor/candidatos/programator/reintentos.py, porque no es documentación: es la entrega real contra la que se verifica el comprobador en tests/puerta_contra_las_entregas_reales.rs. Sin ella, quien clone el repositorio se encontraría la suite en rojo.


Y después de la evaluación del 22/09: cómo se comportó en uso real

La evaluación de arriba medía capacidad, con encargos preparados y pruebas adversarias. Lo que hace en un proyecto de verdad es otra pregunta, y se midió aparte: 14 encargos sobre un proyecto real (Godot + C#) entre el 22 y el 24 de septiembre de 2026, todos verificados contra el código. De ahí salieron el balance por tipo de tarea, los modos de fallo con su antídoto, qué conviene encargarle y qué no (lo resume el apartado «Para qué no sirve» del README de la raíz), y varios cambios al propio Programator deducidos de fallos observados.
