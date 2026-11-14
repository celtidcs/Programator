Los encargos del banco de modelos

Cómo añadir, cambiar o quitar encargos y lenguajes, paso a paso y con un ejemplo de PHP, se explica en el documento docs/Como adaptar el banco de modelos.md.

Cada carpeta es un encargo. El banco lo publica en el canal, deja que Programator lo atienda con el modelo cargado y después lo puntúa.

Los encargos se publican para que cualquier usuario pueda probar un modelo; las tandas (resultados/) no se publican.


Qué hay en cada carpeta

El fichero enunciado.md es el cuerpo del encargo, sin encabezado. El encabezado «## Para <agente>: <id>» lo pone el banco con el nombre del agente configurado.

El fichero ficha.toml dice cómo se comprueba y se puntúa, y se describe abajo.

La carpeta pruebas/ es el proyecto donde se colocan la entrega y las pruebas ocultas. Se copia entera a la carpeta de comprobación de la tanda y allí se ejecutan las órdenes de la ficha, desde su raíz y con bash.

La carpeta referencia/ guarda una solución escrita a mano que debe sacar un 5. Si no lo saca, las pruebas están mal, no la referencia.

El fichero rubrica.md dice qué vale cada nota. Es obligatorio en los de criterio y diagnóstico, que puntúa el agente que dirige. En los de código se usa solo cuando las pruebas ocultas no dan nota (apagadas, o sin poder ejecutarse): el banco deja una ficha con el código entregado y quien dirige lo valora leyéndolo. Esa nota no es comparable con la de las pruebas, y el informe lo dice. Cómo escribirla se explica en el apartado 6 del documento docs/Como adaptar el banco de modelos.md.

La carpeta partida/, en los de auditoría, contiene los ficheros que el modelo tiene que revisar. Se copian a la carpeta de trabajo antes de publicar el encargo.


La ficha

Una ficha completa tiene esta forma:

    id = "E03"
    titulo = "Rust: expandir una cadena de rangos"
    familia = "oficio"            # oficio, criterio o uso
    clase = "funcion_acotada"     # solo en la familia uso; ver abajo
    entrega = "rangos.rs"         # el fichero que debe entregar con escribir_propuesta
    colocar_en = "src/rangos.rs"  # dónde se copia la entrega dentro de pruebas/
    herramientas = ["rust"]       # de serie: rust, python, node, dotnet, lua, sqlite; cualquier
                                  # otro se declara en [comprobacion_local.herramientas]

    [preparar]                    # opcional; lo único con red, ver abajo
    orden = "npm install --silent --no-audit --no-fund"

    [compilar]                    # opcional; si falla, la entrega no compila y la nota es 0
    orden = "cargo build --quiet --tests"

    [[comprobaciones]]            # cada una pasa si su orden termina con código 0
    nombre = "caso_basico_del_enunciado"
    orden = "cargo test --quiet --test adversarias -- --exact caso_basico_del_enunciado"

    [[texto]]                     # requisitos que se comprueban buscando en la entrega
    nombre = "sin unwrap fuera de las pruebas"
    patron = ".unwrap()"
    maximo = 0                    # o minimo = N
    antes_de = "#[cfg(test)]"     # opcional: solo se busca en el texto anterior a esta marca
    ignorar_mayusculas = true     # opcional, por defecto false

El nombre de cada comprobación aparece literalmente en sus pruebas. Así una errata en la ficha no se convierte en una comprobación que pasa siempre porque no ejecuta nada.

El código del modelo no tiene red. La sección compilar y las comprobaciones ejecutan ese código. En la comprobación local corren sin red en Linux y vigilados en Windows, y si intentan conectarse fuera del equipo, el paso falla aunque la orden saliera bien: compilar da «no compila» y una comprobación cuenta como fallida. La terminal lo dice en el momento, con el encargo y la semilla, y el informe lo anota. Por eso lo que descarga dependencias va en [preparar], que corre antes de compilar, con red y sin vigilar. Si falla, la entrega queda «no evaluada»: no se llegó al código del modelo.

Un encargo cuyo código tenga que conectarse a algo lo hace contra un servidor que levantan sus propias pruebas en 127.0.0.1. Esas conexiones no cuentan como intento de red.

En los encargos de auditoría, la ficha lleva además el fichero auditado y los defectos sembrados:

    auditado = "Pedidos.cs"       # ruta dentro de la carpeta de trabajo

    [[defectos]]
    lineas = [41, 62]             # cualquier línea donde se pueda señalar el defecto
    tipo = "DUPLICADO"
    descripcion = "el descuento de socio, copiado en dos métodos"

Lo que hay en partida/ se copia a la raíz de la carpeta de trabajo, conservando las subcarpetas. Un hallazgo acierta si cita una línea a menos del margen configurado de alguna de las «lineas» de un defecto, y cada defecto cuenta una sola vez. El tipo no cuenta para el acierto: se guarda para el informe. El fichero auditado tiene que estar limpio fuera de los defectos sembrados, porque cualquier otro hallazgo cuenta como inventado.

Las clases de la familia uso son las filas de desempeño (FilaDeDesempeno en src/banco/resultados.rs), las mismas que el registro de modelos y la guía: formas, validaciones, funcion_acotada, diagnostico y fichero_limpio.


De dónde sale cada encargo

E01 a E09 son los de la evaluación del 22/09/2026, recuperados del commit anterior a d14fc16. Los enunciados se han precisado donde el original dejaba abierto algo que las pruebas necesitan saber, por ejemplo la forma exacta del tipo Resultado en E04 o el constructor en E06. Por eso las notas de este banco no se comparan con las de aquel día.

Las pruebas ocultas de aquel día se escribieron a medida de lo que entregó Devstral: usaban los nombres de variantes que eligió él y llamaban a sus propias pruebas. Aquí se han reescrito para que exijan solo lo que dice el enunciado.

E10 se ha sustituido por E10a, E10b y E10c, porque la función inventada del original aparece ya en documentos que se publican. Las tres llevan variante_de = "E10" en la ficha: el banco pasa todas las variantes con cada semilla y las puntúa juntas, como E10: su nota es la media de todas. Antes de añadir una variante nueva se comprueba en la documentación de esa biblioteca que la función no existe, y se anota en la ficha cómo se comprobó.


Lo que necesita el equipo que comprueba

Las órdenes de las fichas se escriben para bash, y dan por hecho estos programas en el PATH: cargo (E03), node y npm (E04), python3 (E05), dotnet con el SDK de .NET 10 (E06), lua5.4 (E07) y sqlite3 con funciones de ventana, es decir, 3.25 o posterior (E08). Si en el equipo se llaman de otra forma, se dice en [comprobacion_local.nombres] de banco.toml y las fichas no cambian.
