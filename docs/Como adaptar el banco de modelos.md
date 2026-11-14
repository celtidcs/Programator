Cómo adaptar el banco de modelos

Esta guía es para cualquiera que use Programator y quiera medir un modelo local en los lenguajes y las tareas que le interesan, por ejemplo PHP. Todo se hace editando ficheros de texto. No hace falta tocar el código de Programator ni recompilar nada.


Apartado 1. Qué es el banco y para qué sirve

El banco de modelos es una colección de encargos con la solución conocida. Programator se los pasa a un modelo local, uno detrás de otro, por el mismo ciclo con que atiende los encargos de verdad, y al final deja una nota por cada clase de trabajo, la velocidad y la memoria que necesitó. Con esas cifras se decide si el modelo sirve para lo que se le va a pedir.

El banco que trae Programator está en la carpeta evaluacion/banco/. Dentro hay cuatro cosas. El fichero banco.toml es la configuración del banco; lo creas tú, y para ello te sirve de guía banco.ejemplo.toml, que está al lado y trae todas las claves, comentadas, con su valor por defecto. La carpeta encargos/ tiene una carpeta por encargo, como E01/, E03/ y las demás. La carpeta resultados/ tiene una carpeta por tanda, y la crea el banco.

Puedes usar ese banco, cambiarlo o hacer uno tuyo en otra carpeta con la misma forma.


Apartado 2. Escribir no es comprobar

Esto es lo más importante de la guía.

Para que el modelo escriba código en un lenguaje no hace falta nada instalado. El modelo escribe texto, y Programator lo guarda. Puedes medir cómo escribe PHP en un equipo sin PHP.

Para que el banco compruebe si ese código funciona sí hace falta. El banco ejecuta unas pruebas que el modelo no ve, las pruebas ocultas, y para ejecutar código PHP hace falta el intérprete de PHP en el equipo.

Programator no sabe nada de ningún lenguaje. Ni de Rust, ni de PHP, ni de C#. Lo único que hace es lanzar con bash las órdenes que dice la ficha de cada encargo, y una orden que termina con código 0 pasa; cualquier otro código, falla. Por eso un lenguaje nuevo se añade sin tocar Programator: basta con escribir sus órdenes.

Cuando una entrega no se puede comprobar, su nota sale «no evaluado», nunca un cero, y el informe dice por qué. Siempre empieza por lo que sí está. Por ejemplo, dice: «el código está escrito y guardado, pero no se ha ejecutado para comprobar si funciona: php no está instalado en este equipo. Para comprobarlo, instálalo; si no quieres comprobar ejecutando, apaga las pruebas ocultas con «activa = false» en la sección [comprobacion_local] de banco.toml».


Apartado 3. Apagar las pruebas ocultas

Las pruebas ocultas van encendidas por defecto, también si banco.toml no dice nada. Mientras estén encendidas pasan tres cosas.

La primera es que hace falta tener instalados los lenguajes de los encargos que pases, los de la lista del apartado 4.

La segunda es que el código que ha escrito el modelo, que nadie ha revisado, se ejecuta en este equipo, dentro de la carpeta de la tanda y aislado: en Windows, con integridad baja y dentro de un Job que le impide escribir fuera de su carpeta y vigila si intenta conectarse; en Linux, sin red. El banco lo recuerda al arrancar.

La tercera es que, en Windows, la primera vez Programator copia el bash de Git (unos 200 MB, las carpetas bin, etc y usr, y crea vacía en la copia la carpeta bin de su entorno, como ucrt64\bin, que el lanzador de Git necesita ver) a interprete-aislado/, dentro de la carpeta del banco, y comprueba con esa copia. El bash de Git no arranca aislado si hay otro bash de la misma instalación abierto, cosa normal si trabajas con agentes en el mismo equipo; la copia no comparte nada con ellos. Se hace sola, antes de cargar el modelo, y se repite solo si cambia el Git de origen, por ejemplo al actualizarlo, o si una versión nueva de Programator cambia lo que se copia. No hay que hacer nada; la carpeta se cambia con la clave copia_del_interprete en la sección [comprobacion_local].

Si no quieres eso, apágalas. Son dos líneas en banco.toml:

    [comprobacion_local]
    activa = false

Con las pruebas apagadas, el modelo escribe el código igual, y queda guardado en la carpeta de la tanda, en trabajo/<encargo>-semilla-<n>/.gestor/candidatos/programator/. No hace falta ningún lenguaje instalado. Los encargos de criterio y de diagnóstico se puntúan igual, con su rúbrica.

Los encargos de código se valoran leyéndolos, con su rubrica.md: el banco deja en rubricas/ una ficha con el código entregado y la rúbrica, y quien dirige (el agente de pago, o tú) le pone la nota sin ejecutarlo. Los siete encargos de código del banco traen la suya. Un encargo de código sin rubrica.md sale «no evaluado», diciendo que las pruebas están apagadas y que falta la rúbrica para valorarlo leyendo.

Lo mismo pasa, con las pruebas encendidas, cuando no pueden dar nota a una entrega: falta su lenguaje en el equipo, falla su [preparar] o una orden no se puede lanzar. Si las pruebas sí dan nota (también un 0 porque no compila), manda esa y no se deja ficha.

Las dos notas no son comparables. La de las pruebas es objetiva: cuántas pasan. La de la rúbrica es el juicio de quien lee el código, que puede no ver un fallo que las pruebas habrían cazado. Por eso resultados.toml guarda con qué se puntuó cada semilla, con «puntuada_con = "pruebas_ocultas"» o «puntuada_con = "rubrica"» (y «puntuada_con = "sin_entrega"» si el modelo no entregó a tiempo, un 0 que se compara con cualquiera de las dos), y el informe lo dice en el detalle del encargo. Para comparar modelos entre sí, que sus tandas se hayan puntuado igual.

Para volver a encenderlas, borra esa línea o pon «activa = true».


Apartado 4. Los lenguajes que vienen de serie

El banco ya sabe comprobar estos seis sin que configures nada. Para cada uno se indica el nombre que se pone en la ficha, en la clave herramientas, la orden que mira si está instalado y los encargos del banco que lo usan.

El primero es rust. La orden que mira si está instalado es «cargo --version» y lo usa el encargo E03.

El segundo es node. La orden es «node --version && npm --version» y lo usa el encargo E04.

El tercero es python. La orden es «python3 --version» y lo usan los encargos E05 y U03.

El cuarto es dotnet. La orden es «dotnet --list-sdks» y lo usa el encargo E06, que es de C#.

El quinto es lua. La orden es «lua5.4 -v» y lo usa el encargo E07.

El sexto es sqlite. La orden es «sqlite3 -version» y lo usa el encargo E08.

Una orden «mira si está instalado» cuando termina con 0 si lo está. El banco las lanza todas al empezar la comprobación y dice en la terminal cuáles no responden. Los encargos de criterio (E01, E02, E09, E10a/b/c) y los de auditoría y diagnóstico (U01, U02, U04, U05) no ejecutan código y no necesitan ninguno.

Cualquier otro lenguaje se añade como se explica en el apartado 7.


Apartado 5. La carpeta de un encargo

Cada encargo es una carpeta dentro de encargos/. El nombre de la carpeta no importa; lo que cuenta es el id de su ficha.

Una carpeta de encargo, por ejemplo encargos/P01/, puede tener estos elementos. El fichero ficha.toml es obligatorio y dice qué es el encargo y cómo se puntúa. El fichero enunciado.md es obligatorio y es lo que lee el modelo. El fichero rubrica.md es obligatorio en los encargos de criterio y diagnóstico, y en los de código sirve para valorarlos sin ejecutar. La carpeta pruebas/ va en los de código y es el proyecto donde se coloca la entrega. La carpeta referencia/ es recomendable en los de código y guarda una solución tuya. La carpeta partida/ va en los de auditoría y tiene los ficheros que el modelo revisa. Cada uno se explica a continuación.

El fichero enunciado.md es el cuerpo del encargo, tal cual lo recibe el modelo. No lleva encabezado: el banco pone delante la línea del canal con el nombre del agente. Conviene decir el nombre exacto del fichero que hay que entregar y que se entrega con escribir_propuesta, como hacen los del banco.

La carpeta pruebas/ se copia entera a una carpeta de comprobación, la entrega del modelo se coloca dentro (donde diga colocar_en) y allí se lanzan las órdenes de la ficha, desde la raíz de esa copia y con bash. El modelo no la ve nunca.

La carpeta referencia/ es una solución escrita a mano que debe pasar todas las pruebas. El banco no la usa; sirve para comprobar tus pruebas antes de fiarte de ellas (apartado 9). Si la referencia no pasa, lo que está mal son las pruebas.

El fichero rubrica.md dice qué vale cada nota, del 0 al 5. El banco deja una ficha de rúbrica por cada pasada en resultados/<tanda>/rubricas/, y quien dirige las puntúa. En los encargos de criterio y diagnóstico es obligatoria. En los de código es opcional pero muy recomendable: solo se usa cuando las pruebas ocultas no dan nota (apartado 3), y entonces la ficha lleva el código entregado en vez de la respuesta. Cómo escribirla se explica en el apartado 6.

La carpeta partida/ se copia a la carpeta de trabajo antes de publicar el encargo, conservando las subcarpetas. Es lo que el modelo tiene que auditar.


Apartado 6. La ficha, clave a clave

El fichero ficha.toml es lo único que el banco interpreta. Una clave que no exista, o mal escrita, es un error al cargar el banco, con el encargo y la clave.


Las tres familias

La clave familia admite tres valores. Con oficio, el encargo mide escribir código y se puntúa a máquina, con pruebas ocultas y requisitos de texto. Con criterio, mide protocolo, diseño, depuración y honestidad, y se puntúa con rubrica.md. Con uso, mide las clases de trabajo reales; lleva la clave clase y se puntúa según la clase.

Las clases de uso son estas. La clase funcion_acotada se puntúa como uno de oficio, con pruebas ocultas. Las clases formas y validaciones se puntúan como una auditoría: cuántos defectos sembrados encuentra y cuántos se inventa. La clase fichero_limpio es la auditoría de un fichero sin defectos, en la que todo hallazgo es inventado. La clase diagnostico se puntúa con rubrica.md.


Claves de todos los encargos

La clave id es obligatoria. Es el identificador único en el banco, por ejemplo «"P01"». Dos iguales son un error.

La clave titulo es obligatoria y sirve para el informe.

La clave familia es obligatoria y vale oficio, criterio o uso.

La clave clase solo se pone en uso, y es una de las de arriba. En oficio y criterio no se pone.

La clave variante_de no es obligatoria. Si el encargo es una de varias versiones de un mismo encargo, lleva el id de ese encargo. El banco pasa todas las variantes con cada semilla y las puntúa juntas, como ese encargo: su nota es la media de todas.


Claves de los encargos de código (oficio y funcion_acotada)

La clave entrega es obligatoria. Es el nombre del fichero que el modelo entrega con escribir_propuesta.

La clave colocar_en es obligatoria. Dice dónde se copia la entrega dentro de pruebas/.

La clave herramientas es obligatoria. Son los lenguajes o programas que hacen falta para comprobarla, por ejemplo «["php"]». Pueden ser de serie o declarados en banco.toml (apartado 7). Se escriben en minúsculas, con letras, dígitos, guion (-) y guion bajo (_).

La clave orden de la sección [preparar] no es obligatoria. Es lo que se lanza antes de compilar y no toca el código del modelo, como descargar dependencias. Es lo único con red. Si falla, la entrega sale «no evaluado».

La clave orden de la sección [compilar] no es obligatoria. Si falla, la entrega «no compila» y su nota es 0.

Las secciones [[comprobaciones]] tienen que ser al menos una. Cada una lleva nombre y orden, y pasa si la orden termina con 0.

Las secciones [[texto]] no son obligatorias. Son requisitos que se buscan en el texto de la entrega, sin ejecutarla.

Cada [[texto]] lleva estas claves. La clave nombre sirve para el informe. La clave patron es el texto que se busca, literal (no es una expresión regular). Las claves maximo o minimo van una de las dos, y solo una: dicen cuántas veces puede aparecer el texto como mucho, o tiene que aparecer como poco. La clave antes_de es opcional: con ella solo se busca en lo que hay antes de esta marca, por ejemplo «"#[cfg(test)]"». La clave ignorar_mayusculas es opcional y por defecto vale false.

Hay dos reglas que evitan pruebas que pasan sin probar nada.

La primera es que el nombre de cada comprobación aparece literalmente en sus pruebas. La orden lo pasa como argumento y el programa de pruebas lo busca; un nombre que no existe tiene que terminar con un código distinto de 0. Así una errata en la ficha no se convierte en una prueba que siempre pasa.

La segunda es que el código del modelo no tiene red. La orden de compilar y las comprobaciones lo ejecutan, y si intenta conectarse fuera del equipo ese paso falla aunque la orden saliera bien. Lo que necesite descargar va en [preparar]. Si las pruebas necesitan un servidor, lo levantan ellas en 127.0.0.1, que sí está permitido.


La rúbrica de un encargo de código

Quien la aplica lee el código, no lo ejecuta. Así que cada criterio tiene que poder decidirse leyendo. Lo que funciona, y es lo que hacen las siete del banco (mira E03/rubrica.md o E07/rubrica.md como modelo), son cuatro cosas.

Primero, sácala de las pruebas ocultas. Cada prueba mira algo; ese algo es un criterio de la rúbrica, dicho de forma que se vea en el código: «rechaza el rango invertido ("5-2")», «usa índices de cabeza y de cola, sin table.remove(t, 1)». Así las dos notas miden lo mismo, aunque no sean comparables.

Segundo, puntos que suman 5, cada uno entero o nada: «1: la firma es exactamente …», «0,5: no crea variables globales». Más valor a lo esencial (la firma, que haga lo pedido) que a los detalles.

Tercero, los topes, aparte: un 0 si con seguridad no compilaría o no respeta la firma (como pasaría con las pruebas), y un máximo si se salta lo esencial (una biblioteca externa en vez de escribirlo). Di también que, si solo se duda de si compila, no se resta por eso.

Cuarto, las trampas del lenguaje, por su nombre: NaN en JavaScript, la división entera en SQL, un as que trunca en Rust. Es lo que un lector con prisa no mira.

En el ejemplo de PHP del apartado 7, una rúbrica breve sería este fichero, escrito en markdown:

    # P01: expandir una cadena de rangos en PHP

    Se usa solo cuando las pruebas ocultas no dieron nota. Se puntúa leyendo el código, sin
    ejecutarlo.

    ## Qué se mira, y cuánto vale

    - **1**: la firma es `function expandir_rangos(string $rangos): array`.
    - **2**: `"1-3,7"` da `[1, 2, 3, 7]`, con los dos extremos de cada rango incluidos.
    - **1**: la cadena vacía da `[]`.
    - **1**: un rango al revés (`"5-2"`) lanza `InvalidArgumentException`.

    ## Qué baja la nota

    - **0**: no entregó código, la función se llama de otra forma o con seguridad no se puede cargar.


Claves de los encargos de auditoría (formas, validaciones, fichero_limpio)

La clave auditado es obligatoria. Es la ruta, dentro de partida/, del fichero que el modelo revisa.

Las secciones [[defectos]] son obligatorias en formas y validaciones, y están prohibidas en fichero_limpio. Cada una lleva lineas (cualquier línea donde se pueda señalar), tipo y descripcion.

Un hallazgo acierta si cita una línea que está a margen_lineas líneas o menos (de banco.toml) de alguna de las lineas de un defecto. El tipo no cuenta para acertar. El fichero auditado tiene que estar limpio fuera de los defectos sembrados, porque cualquier otro hallazgo cuenta como inventado.


Encargos de criterio y diagnóstico

Solo necesitan id, titulo, familia (y «clase = "diagnostico"» en uso), enunciado.md y rubrica.md. Por ejemplo:

    id = "E01"
    titulo = "Protocolo y autoconocimiento"
    familia = "criterio"


Apartado 7. Añadir un lenguaje: ejemplo completo con PHP

Vamos a añadir un encargo que mide si el modelo escribe bien una función en PHP.


Paso 1. Decirle al banco cómo saber si PHP está instalado

En banco.toml:

    [comprobacion_local.herramientas]
    php = "php --version"

La palabra php es el nombre que usarán las fichas; la orden «php --version» termina con 0 si PHP está instalado. Si una ficha pide php y esta línea no está, el banco no arranca, antes de cargar el modelo, y dice qué encargo lo pide y qué línea añadir.

Si en este equipo el programa se llama de otra forma, por ejemplo php8.3, pon esa orden aquí y mira el apartado 9 para que las fichas no cambien.


Paso 2. La carpeta del encargo

La carpeta encargos/P01/ lleva los ficheros ficha.toml, enunciado.md y rubrica.md, este último para valorarlo leyendo si las pruebas no dan nota (apartado 6). Además lleva la carpeta pruebas/, con el fichero comprobar.php dentro, y la carpeta referencia/, con el fichero rangos.php dentro.


Paso 3. El fichero ficha.toml

    id = "P01"
    titulo = "PHP: expandir una cadena de rangos"
    familia = "oficio"
    entrega = "rangos.php"
    colocar_en = "rangos.php"
    herramientas = ["php"]

    # Si no tiene errores de sintaxis, compila. Si los tiene, la nota es 0.
    [compilar]
    orden = "php -l rangos.php"

    [[comprobaciones]]
    nombre = "caso_basico"
    orden = "php comprobar.php caso_basico"

    [[comprobaciones]]
    nombre = "un_solo_numero"
    orden = "php comprobar.php un_solo_numero"

    [[comprobaciones]]
    nombre = "cadena_vacia"
    orden = "php comprobar.php cadena_vacia"

    [[comprobaciones]]
    nombre = "rango_al_reves_lanza_excepcion"
    orden = "php comprobar.php rango_al_reves_lanza_excepcion"

    [[texto]]
    nombre = "tipos estrictos"
    patron = "declare(strict_types=1)"
    minimo = 1


Paso 4. El fichero enunciado.md

    Encargo de código. Entrégalo con escribir_propuesta, nombre exacto rangos.php

    Escribe en PHP 8 una función que expanda una cadena de rangos:

    - Firma: function expandir_rangos(string $rangos): array
    - "1-3,5" devuelve [1, 2, 3, 5]; "7" devuelve [7]; "" devuelve [].
    - Un rango al revés, como "5-1", lanza InvalidArgumentException.
    - El fichero empieza con declare(strict_types=1).

    Publica un resumen de lo entregado.


Paso 5. Las pruebas ocultas: el fichero pruebas/comprobar.php

Es un programa que recibe el nombre de una prueba, la ejecuta y termina con 0 si pasa, 1 si falla y 2 si el nombre no existe:

    <?php
    // Pruebas que el modelo no ve, sobre su rangos.php.
    // Uso: php comprobar.php <nombre>. Termina con 0 si pasa, 1 si falla y 2 si el nombre no existe.
    declare(strict_types=1);

    require __DIR__ . '/rangos.php';

    $pruebas = [
        'caso_basico' => fn () => expandir_rangos('1-3,5') === [1, 2, 3, 5],
        'un_solo_numero' => fn () => expandir_rangos('7') === [7],
        'cadena_vacia' => fn () => expandir_rangos('') === [],
        'rango_al_reves_lanza_excepcion' => function (): bool {
            try {
                expandir_rangos('5-1');
                return false;
            } catch (InvalidArgumentException) {
                return true;
            }
        },
    ];

    $nombre = $argv[1] ?? '';
    if (!isset($pruebas[$nombre])) {
        fwrite(STDERR, "no existe la prueba «{$nombre}»\n");
        exit(2);
    }
    exit($pruebas[$nombre]() ? 0 : 1);

Si la entrega del modelo da un error fatal, PHP termina con un código distinto de 0 y esa prueba cuenta como fallida, que es lo que debe pasar.


Paso 6. La referencia: el fichero referencia/rangos.php

    <?php
    declare(strict_types=1);

    function expandir_rangos(string $rangos): array
    {
        if ($rangos === '') {
            return [];
        }
        $numeros = [];
        foreach (explode(',', $rangos) as $parte) {
            [$desde, $hasta] = array_pad(explode('-', $parte), 2, $parte);
            if ((int) $desde > (int) $hasta) {
                throw new InvalidArgumentException("rango al revés: {$parte}");
            }
            array_push($numeros, ...range((int) $desde, (int) $hasta));
        }
        return $numeros;
    }


Paso 7. Probarlo y pasar el banco

Comprueba las pruebas con la referencia antes de gastar una tanda (apartado 9) y después lanza el banco (apartado 10). El encargo P01 aparece en el informe como uno más.


Apartado 8. Con cualquier otro lenguaje se hace igual

Lo que cambia de un lenguaje a otro son las órdenes de la ficha y el programa de pruebas/. El banco solo lanza esas órdenes y mira si terminan con 0. Siguen dos ejemplos breves.


C#

El encargo E06 del banco es un modelo completo. Pide «herramientas = ["dotnet"]», que viene de serie, y su carpeta pruebas/ es un proyecto de consola (banco_e06.csproj y Program.cs) que recibe el nombre de la prueba igual que el comprobar.php de arriba:

    [preparar]
    orden = "dotnet restore --nologo -v q"

    [compilar]
    orden = "dotnet build --no-restore --nologo -v q -c Release -o salida"

    [[comprobaciones]]
    nombre = "lotea_siete_lineas_de_tres_en_tres"
    orden = "dotnet salida/banco_e06.dll lotea_siete_lineas_de_tres_en_tres"


Visual Basic

Necesita lo mismo que C#, el SDK de .NET. Puedes pedir dotnet en la ficha, o darle su propio nombre para que el informe y los avisos digan «visualbasic». En banco.toml:

    [comprobacion_local.herramientas]
    visualbasic = "dotnet --list-sdks"

La carpeta pruebas/ lleva un proyecto de consola de Visual Basic, banco_vb.vbproj:

    <Project Sdk="Microsoft.NET.Sdk">
      <PropertyGroup>
        <OutputType>Exe</OutputType>
        <TargetFramework>net10.0</TargetFramework>
        <AssemblyName>banco_vb</AssemblyName>
      </PropertyGroup>
    </Project>

y un Program.vb que recibe el nombre de la prueba y termina con 0, 1 o 2, como el de PHP. La ficha:

    id = "V01"
    titulo = "Visual Basic: una calculadora"
    familia = "oficio"
    entrega = "Calculadora.vb"
    colocar_en = "Calculadora.vb"
    herramientas = ["visualbasic"]

    [preparar]
    orden = "dotnet restore --nologo -v q"

    [compilar]
    orden = "dotnet build --no-restore --nologo -v q -c Release -o salida"

    [[comprobaciones]]
    nombre = "suma_dos_numeros"
    orden = "dotnet salida/banco_vb.dll suma_dos_numeros"


Apartado 9. Cambiar, renombrar o quitar


Cambiar la orden de un lenguaje de serie

Se hace en [comprobacion_local.herramientas], con el mismo nombre. Por ejemplo, si tu Lua es lua y no lua5.4:

    [comprobacion_local.herramientas]
    lua = "lua -v"


Un programa que aquí se llama distinto

Las fichas del banco llaman a los programas como se llaman en Linux (python3, lua5.4). Si en tu equipo se llaman de otra forma, no cambies las fichas: dilo en [comprobacion_local.nombres] y el banco crea, antes de cada orden, una función de bash con el nombre de la ficha que llama al tuyo:

    [comprobacion_local.nombres]
    python3 = "python"
    "lua5.4" = "lua"

Las órdenes de [comprobacion_local.herramientas] pasan también por estos nombres, así que con «python3 = "python"» sigue valiendo la orden de serie.


Quitar un lenguaje

Quita o mueve fuera de encargos/ las carpetas de los encargos que lo piden. Si era uno que declaraste tú, quita también su línea de [comprobacion_local.herramientas]. Los de serie no hace falta quitarlos: si no están instalados, el banco lo avisa y sigue.


Quitar un encargo

Borra su carpeta de encargos/ o muévela fuera. El banco pasa lo que encuentre en encargos/. Las tandas ya guardadas no cambian: cada una conserva lo que se pasó.


Comprobar tus pruebas con la referencia

Antes de fiarte de un encargo nuevo, copia su carpeta pruebas/ a una carpeta aparte, coloca ahí tu referencia donde diga colocar_en y lanza a mano, con bash, cada orden de la ficha. Todas deben terminar con 0. Con un nombre de prueba inventado debe terminar con otro código.


Apartado 10. Lanzar el banco y ver los resultados

Hay cinco órdenes. La primera pasa una tanda entera: los encargos con el modelo y después las pruebas ocultas.

programator --banco <carpeta del banco>

La segunda hace lo mismo con otro modelo, sin tocar programator.toml.

programator --banco <carpeta> --modelo <fichero.gguf>

La tercera repite solo las pruebas ocultas de una tanda ya pasada, sin cargar el modelo. Sirve, por ejemplo, después de instalar un lenguaje que faltaba. Usa las fichas y el banco.toml de ahora.

programator --banco-comprobar <carpeta de la tanda>

La cuarta rehace el informe cuando las rúbricas están puntuadas.

programator --banco-informe <resultados.toml>

La quinta dice si dos tandas son repetición una de otra.

programator --banco-comparar <tanda A> <tanda B>

Cada tanda deja una carpeta en resultados/ con cinco cosas. El fichero informe.md es para leerlo: tiene una fila por encargo, con su nota y, en «Detalle», el motivo de cada «no evaluado». El fichero resultados.toml tiene todas las cifras, para otros programas. El fichero pasadas.toml guarda lo que se pasó con el modelo, y es lo que usa --banco-comprobar para repetir las pruebas ocultas. La carpeta rubricas/ guarda las fichas que quedan por puntuar. La carpeta trabajo/ guarda lo que pasó en cada encargo, con la entrega del modelo en trabajo/<encargo>-semilla-<n>/.gestor/candidatos/programator/.


¿Se puede comprobar la configuración sin cargar el modelo?

No hay una orden que solo compruebe. Pero --banco lee banco.toml y todas las fichas antes de cargar el modelo, y se para en el primer problema diciendo cuál: una clave mal escrita, una ficha a la que le falta algo, un lenguaje sin orden. Un error de configuración sale en segundos.
