# Norma de integración de aviso-version-github

Obligatoria para todo programa que use esta biblioteca. Todos los programas la conectan **de la misma forma**, para que un agente que conoce una integración las conozca todas. La referencia ejecutable es `examples/integracion_de_referencia.rs`: Cargo la compila, clippy la revisa y `cargo test` ejecuta sus pruebas.

- Para qué es cada función de la biblioteca: `guia-de-uso-para-agentes.md`.
- Dudas y fallos habituales: `problemas-frecuentes.md`.
- Qué programas la usan y qué fallos han encontrado: el registro común, `registro de proyectos_bibliotecas.md`, en la carpeta que contiene el repositorio de la biblioteca (no va con las copias).

Si un programa necesita apartarse de una regla, la desviación se anota en su apartado del registro común, con su motivo. Si no está anotada, la desviación es un defecto.

## N1. La copia de la biblioteca

1. Va en `bibliotecas/aviso-version-github/`, en la raíz del programa. Ninguna otra ruta.
2. Es una copia íntegra de una versión etiquetada de este repositorio (`vX.Y.Z`): `Cargo.toml`, `Cargo.lock`, `LICENSE`, `README.md`, `src/`, `examples/` y `documentacion/`. Sin `target/`.
3. No se edita dentro del programa. Un fallo o una necesidad se anota en el registro común; el cambio se hace en este repositorio, se etiqueta y se vuelve a copiar.
4. Para saber qué versión lleva un programa, mira el `version` de `bibliotecas/aviso-version-github/Cargo.toml`.

## N2. La dependencia

1. Se declara por ruta, en el `Cargo.toml` del crate que contiene el módulo de N3:

```toml
aviso-version-github = { path = "bibliotecas/aviso-version-github" }
```

   (en un workspace, la ruta relativa desde ese crate, como `../../bibliotecas/aviso-version-github`).
2. Si el programa es un workspace, la copia es miembro del workspace. Así sus pruebas corren con las del programa.
3. Si no lo es, las puertas del programa añaden `cargo test --manifest-path bibliotecas/aviso-version-github/Cargo.toml`.
4. No se añade `ureq` aparte para hablar con GitHub. Si hace falta un proxy fijo, se usa el reexportado `aviso_version_github::ureq`.

## N3. Un solo módulo conecta la biblioteca

1. El programa tiene **un** módulo, llamado `aviso_de_version`, que es el único que llama a `comprobar`, `ultima_version`, `ConsultaHttps` y `OrigenDeLasVersiones`. Se copia de `examples/integracion_de_referencia.rs`.
2. Fuera de ese módulo solo se usan dos cosas: el tipo de las preferencias (`PreferenciasDeVersion`, o sus dos campos dentro de las preferencias del programa) y el resultado (`AvisoDeVersionNueva`).
3. El módulo tiene dos constantes con estos nombres:
   - `NOMBRE_DEL_PROGRAMA`: nombre para la cabecera User-Agent, como `mi-programa`. En minúsculas, con guiones y sin espacios.
   - `REPOSITORIO_DE_VERSIONES_POR_DEFECTO`: el `dueño/nombre` donde se publican las versiones del programa.

## N4. La configuración

1. Las preferencias del programa llevan dos campos con estos nombres exactos:
   - `comprobar_version_nueva: bool`. Por defecto `true`. El usuario puede desactivarlo.
   - `ajustes_de_version: AjustesDeVersionNueva`. Por defecto, `AjustesDeVersionNueva::para_repositorio(REPOSITORIO_DE_VERSIONES_POR_DEFECTO)`.
2. Los dos llevan `#[serde(default = ...)]`, para que unas preferencias antiguas sin ellos sigan leyéndose.
3. Los límites no se fijan en el código. El tiempo de espera y el tamaño de respuesta salen de `ajustes_de_version`. Si el programa quiere otro valor de fábrica, lo cambia en su función de ajustes de fábrica, no al construir la consulta.
4. Un programa con configuración TOML usa la sección `[aviso_de_version]` con esos dos campos. En JSON se usan los mismos nombres.

## N5. Construir la consulta

Siempre en este orden, como en `arrancar` de la referencia:
1. Si `comprobar_version_nueva` es `false`, se termina sin crear nada.
2. Se llama a `OrigenDeLasVersiones::desde_ajustes(&ajustes_de_version)`. Si da `Err`, se registra (N7) y se termina sin consultar.
3. Se fabrica la consulta con la función inyectada `fabricar(&origen)`. La real es `consulta_real`: `ConsultaHttps::para_programa(&origen, proxy_del_sistema(), &programa)`, con `programa = ProgramaQueConsulta::nuevo(NOMBRE_DEL_PROGRAMA, env!("CARGO_PKG_VERSION"))`.
4. La versión local es siempre `env!("CARGO_PKG_VERSION")` del crate del programa, nunca un texto escrito a mano.

Prohibido: construir `ConsultaHttps` sin `ProgramaQueConsulta` (salvo el respaldo de `consulta_real`), componer la dirección de la API a mano y leer el proxy por otro camino.

## N6. Cuándo y en qué hilo

1. Una comprobación al arrancar el programa. Opcionalmente, otra cuando el usuario lo pida desde un menú. Nunca en un bucle ni con temporizador: el límite de peticiones sin autenticar de GitHub es bajo.
2. Siempre en un hilo de trabajo con nombre (`aviso-de-version`). El resultado llega por un `mpsc::Receiver`.
3. El hilo de la interfaz nunca espera a la red. Recoge el resultado con `try_recv` en su bucle.
4. Un programa de consola sin interfaz puede esperar con `recv`, pero solo después de haber hecho su trabajo principal o en paralelo a él, nunca antes.

## N7. Fallos

1. Todo `FalloDeComprobacion` se registra con el sistema de registro del programa, a nivel informativo o de aviso, con el texto de su `Display`.
2. Prohibido descartarlo en silencio (`.ok()`, `let _ =` o `unwrap_or_default()` sin registrar).
3. Prohibido mostrarlo al usuario como un error o en un diálogo: si falla, simplemente no se avisa.
4. Prohibido convertirlo en pánico (`unwrap`, `expect`, `panic!`).

## N8. Presentar el aviso

1. Con `Some(aviso)`, se muestra un aviso discreto con `aviso.version` y un enlace a `aviso.enlace`.
2. El enlace solo se abre cuando el usuario lo pulsa. El programa no descarga ni instala nada.
3. Los textos del aviso viven en el módulo de textos del programa, con el resto de textos traducibles.
4. El enlace se usa tal como llega. No se compone ni se modifica.

## N9. La versión de otro programa

Cuando un programa quiere mostrar la última versión de otro programa:
1. Usa `ultima_version(&origen, consulta)` con un origen propio para ese repositorio. No vale componer `pedir`, `url_de_consulta` e `interpretar_respuesta` a mano.
2. El repositorio del otro programa es otro campo de configuración con su propio valor de fábrica, en la misma sección.
3. Mismas reglas de hilo (N6) y de fallos (N7).

## N10. Pruebas mínimas del programa

Con una consulta falsa (`ConsultaDeVersion`) y sin red, el programa prueba como mínimo estos cinco casos, que son los de la referencia:
1. Con la opción desactivada, no se fabrica la consulta.
2. Con ajustes no válidos, no se fabrica la consulta y el fallo se registra.
3. Una versión publicada mayor llega como aviso.
4. La misma versión no da aviso ni registra nada.
5. Un fallo de red no da aviso, se registra y no provoca pánico.

Además: `NOMBRE_DEL_PROGRAMA` es válido para `ProgramaQueConsulta::nuevo`, y `REPOSITORIO_DE_VERSIONES_POR_DEFECTO` supera `desde_ajustes`.

## N11. Actualizar la copia

1. Copia la versión etiquetada nueva sobre `bibliotecas/aviso-version-github/` (N1).
2. Lee su entrada en el historial de cambios del `README.md` y, si rompe la compatibilidad, el apartado de migración de `guia-de-uso-para-agentes.md`.
3. Adapta el módulo `aviso_de_version` comparándolo con el nuevo `examples/integracion_de_referencia.rs`.
4. Pasa las puertas del programa en Linux y Windows, incluidas las pruebas de la copia (N2).
5. Pon al día el apartado del programa en el registro común: versión de la copia, fecha, desviaciones y puertas pasadas.
6. Revisa la documentación del programa que hable del aviso de versión: manual, configuración e historial de cambios.

## Lista de comprobación para aceptar una integración

Quien verifica marca cada punto ejecutando o mirando el archivo, nunca de memoria:

- [ ] N1: `bibliotecas/aviso-version-github/` es idéntica, archivo por archivo, a la etiqueta que dice su `Cargo.toml` (compara con `sha256sum`).
- [ ] N2: dependencia por ruta; las pruebas de la copia corren en las puertas.
- [ ] N3: un `grep` de `aviso_version_github::` fuera del módulo `aviso_de_version` solo encuentra los tipos permitidos.
- [ ] N4: los dos campos con sus nombres, sus valores por defecto y `serde(default)`.
- [ ] N5: `consulta_real` usa `para_programa` y `proxy_del_sistema`; la versión local es `env!("CARGO_PKG_VERSION")`.
- [ ] N6: la llamada ocurre en un hilo con nombre; ningún `recv` bloqueante en el hilo de la interfaz.
- [ ] N7: un `grep` de `.ok()` sobre resultados de la biblioteca no encuentra descartes sin registrar.
- [ ] N8: el enlace solo se abre por acción del usuario.
- [ ] N9: si consulta otro programa, usa `ultima_version`.
- [ ] N10: las cinco pruebas existen y fallan si se rompe lo que vigilan.
- [ ] N11: el apartado del programa en el registro común está al día.
