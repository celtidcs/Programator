aviso-version-github

Biblioteca en Rust que comprueba si hay una versión más nueva de un programa publicada en GitHub o en una instalación propia de GitHub Enterprise Server. Pregunta una vez a la API cuál es la última versión publicada de un repositorio y, si es más nueva que la que se está ejecutando, devuelve un aviso con el número de versión y el enlace a su página. No descarga ni instala nada: abrir el enlace es decisión de quien usa el programa.

Qué hace

La biblioteca hace tres cosas, cada una en su propio módulo. Lee y valida los ajustes, que dicen de qué repositorio y de qué servidor se pregunta y con qué límites. Hace la consulta por HTTPS a la operación de la API que devuelve la última versión publicada, que excluye las versiones preliminares y los borradores. Y decide si hay que avisar comparando la versión publicada con la local como números, de modo que 0.12.0 es posterior a 0.9.9. La consulta está detrás de un rasgo, ConsultaDeVersion, para poder probar la decisión sin red con una consulta falsa.

Cómo se usa

Cada programa lleva su propia copia en la carpeta bibliotecas/aviso-version-github y la declara como dependencia por ruta en su Cargo.toml, con esta línea:

aviso-version-github = { path = "bibliotecas/aviso-version-github" }

Después, el programa prepara los ajustes con AjustesDeVersionNueva::para_repositorio, pasándole su repositorio con la forma dueño/nombre; los valida con OrigenDeLasVersiones::desde_ajustes; crea la consulta con ConsultaHttps::nueva, pasándole el origen ya validado y el resultado de proxy_del_sistema; y llama a comprobar con su propia versión, por ejemplo la que da env!("CARGO_PKG_VERSION"). El resultado es Ok(Some(aviso)) si hay una versión más nueva, Ok(None) si no la hay, y Err con un FalloDeComprobacion si algo falló. La llamada espera a la red, así que un programa con interfaz gráfica debe hacerla en un hilo de trabajo y recoger el resultado cuando llegue.

Si el programa solo quiere saber cuál es la última versión publicada, sin compararla con la suya, llama a ultima_version con el origen y la consulta, y recibe una VersionSemantica. Y si quiere presentarse ante GitHub con su nombre y su versión, crea un ProgramaQueConsulta y prepara la consulta con ConsultaHttps::para_programa; la cabecera User-Agent queda entonces como mi-programa/1.2.0 seguido de aviso-version-github y su versión.

La carpeta documentacion reúne todo lo necesario para conectar la biblioteca de la misma forma en cualquier programa. Su punto de entrada es indice.md. La norma de integración fija la forma única de hacerlo, y el ejemplo integracion_de_referencia de la carpeta examples la aplica en código: Cargo lo compila y cargo test ejecuta sus pruebas. La carpeta también tiene la guía de la API y los problemas frecuentes. Qué programas usan la biblioteca, y los fallos que encuentran en ella, constan en el registro común de proyectos y bibliotecas, un archivo aparte que se guarda junto al repositorio de la biblioteca y no va con las copias.

Configuración

Los ajustes tienen cinco campos y se pueden guardar en las preferencias del programa, porque se leen y se escriben con serde. El campo repositorio es el dueño/nombre del repositorio y no tiene valor por defecto: la biblioteca no conoce ningún repositorio, y unos ajustes sin él no superan la validación. El campo url_api es la raíz de la API y vale https://api.github.com por defecto. El campo url_web es la raíz de la web, de la que sale el enlace del aviso, y vale https://github.com por defecto. El campo segundos_de_espera es el tiempo máximo de espera de la respuesta y vale 10 segundos. El campo limite_de_respuesta_en_bytes es el tamaño máximo que se acepta leer y vale un mebibyte, 1048576 bytes; la respuesta real ocupa unos pocos kilobytes. Al leer unos ajustes guardados, cada campo que falte toma su valor por defecto, salvo el repositorio.

Seguridad

Los ajustes son texto que cualquiera puede editar, así que se validan antes de usarse. Las dos direcciones solo se aceptan con https, con un nombre de servidor válido, sin credenciales ni puerto, y como mucho con una ruta limpia. El nombre del servidor se comprueba parte por parte: cada parte entre puntos tiene de 1 a 63 letras, números o guiones, sin guion al principio ni al final, y el nombre entero no pasa de 253 caracteres, como fija el RFC 1035. El repositorio solo se acepta con la forma dueño/nombre y caracteres que GitHub admite. Un tiempo de espera o un límite de respuesta de cero también se rechazan. La consulta solo se puede preparar a partir de un origen ya validado, así que unos ajustes sin validar nunca llegan a la red. El nombre con que se presenta el programa solo admite letras, números, puntos, guiones y guiones bajos, para que no pueda colar otra cabecera en la petición.

La respuesta del servidor tampoco se da por buena. De ella solo se toma la etiqueta de la versión, y solo si tiene exactamente la forma X.Y.Z, con una v opcional delante; cualquier otra cosa, como una versión preliminar con -rc1, es un fallo y no produce aviso. El enlace del aviso nunca sale de la respuesta, aunque traiga uno: se compone con la dirección web y el repositorio de los ajustes y con la etiqueta ya validada, que solo contiene números, puntos y quizá una v. La consulta usa rustls con las raíces de certificados de webpki-roots, rechaza cualquier dirección que no sea https y no lee más bytes de los que permite el límite. Ningún fallo provoca un pánico: todos llegan como valor, para que un programa compilado con panic = "abort" no se cierre por una consulta que sale mal. La biblioteca prohíbe el código inseguro y, fuera de las pruebas, las llamadas que pueden acabar en pánico, y es el compilador quien lo vigila.

Proxy

La consulta sale por el proxy que se le da a ConsultaHttps::nueva. La función proxy_del_sistema devuelve el que corresponde según el sistema. Primero mira las variables de entorno ALL_PROXY, HTTPS_PROXY y HTTP_PROXY, en mayúsculas o en minúsculas, con las excepciones de NO_PROXY. Si no hay ninguna y el programa corre en Windows, usa el proxy activado en las opciones de internet del sistema, con sus excepciones. Las variables, cuando existen, siempre mandan. Hay tres casos que no cubre: no interpreta archivos de configuración automática del proxy, PAC o WPAD; no se autentica ante el proxy con NTLM ni con Kerberos; y fuera de Windows no lee ninguna configuración del escritorio. Detrás de un proxy así, la solución es definir HTTPS_PROXY con la dirección del proxy antes de abrir el programa.

GitHub Enterprise Server

Una instalación propia de GitHub Enterprise Server publica su API en una dirección con ruta, del tipo https://servidor/api/v3. Para usarla, se pone esa dirección en url_api y la de la web, https://servidor, en url_web. La ruta solo puede tener tramos de letras, números, puntos, guiones y guiones bajos; se rechazan los tramos vacíos, los retrocesos de un punto o de dos, las consultas, los fragmentos, los caracteres escapados, las barras invertidas, los espacios y los caracteres de control, para que ninguna dirección pueda apuntar a otro sitio del que parece.

Cómo se compila y se prueba

Se ha probado con Rust 1.99 en Windows y en Linux; no se ha medido con versiones anteriores. Desde la carpeta de la biblioteca se compila con la orden

cargo build

y se prueba con la orden

cargo test

Ninguna prueba necesita red, ni el registro de Windows, ni variables de entorno. Antes de dar por bueno un cambio conviene pasar también las demás comprobaciones, cada una con su orden:

cargo clippy --all-targets -- -D warnings

cargo fmt --check

cargo doc --no-deps

cargo audit

Una de las pruebas lee este documento y los comentarios de documentación del código, y falla si encuentra marcas de Markdown o referencias al proceso con que se escribió.

Licencia

GPL-3.0 o posterior, la misma que el programa del que se separó. El texto íntegro está en el archivo LICENSE.

Historial de cambios

Versión 0.2.3, del 23 de noviembre de 2026. Ningún archivo de la biblioteca contiene ya datos del equipo en que se desarrolla: ni rutas, ni nombres de personas, programas o máquinas, ni el correo personal del autor, que pasa a ser el que asigna GitHub. Una prueba nueva lo vigila en todos los archivos que se versionan, no solo en la documentación. El código y su comportamiento no cambian.

Versión 0.2.2, del 23 de noviembre de 2026. Solo cambia la documentación: ya no cita rutas del equipo en que se escribe ni el nombre del programa del que se separó, para que pueda publicarse con la copia de cualquier programa. Una prueba nueva vigila que los documentos de la carpeta documentacion tampoco las citen. El código y su comportamiento no cambian.

Versión 0.2.1, del 23 de noviembre de 2026. Solo cambia la documentación: el apartado de uso del README indica la ruta de la copia que lleva cada programa, bibliotecas/aviso-version-github, como manda la norma de integración, en lugar de una carpeta hermana. El código y su comportamiento no cambian.

Versión 0.2.0, del 23 de noviembre de 2026. Rompe la compatibilidad en dos puntos: ConsultaHttps::nueva recibe ahora el origen ya validado en lugar de los ajustes, y FalloDeComprobacion tiene una variante nueva, IdentificacionNoValida. Añade ultima_version, para pedir la última versión sin compararla, y ProgramaQueConsulta con ConsultaHttps::para_programa, para que el programa se identifique en la cabecera User-Agent. Valida el nombre del servidor parte por parte, de modo que ahora se rechazan nombres como a..b o -a.com, que antes se aceptaban. Y prohíbe en la propia biblioteca el código inseguro y las llamadas que pueden acabar en pánico.

Versión 0.1.0, del 18 de noviembre de 2026. Primera versión como biblioteca independiente, separada del aviso de versión nueva del programa en que nació. Respecto a aquel código añade dos cosas: en Windows lee el proxy de las opciones de internet cuando no hay variables de entorno de proxy, y admite la dirección con ruta de la API de GitHub Enterprise Server.
