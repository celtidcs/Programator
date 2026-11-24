# Guía de uso de aviso-version-github para agentes

Referencia de la API para el agente que integra esta biblioteca en un programa en Rust o mantiene esa integración. Describe la versión 0.2.3. **Cómo** se conecta en un programa lo fija `norma-de-integracion.md`, que es obligatoria; el punto de entrada de toda la documentación es `indice.md`. La referencia completa de cada función está en los comentarios de documentación del código (`cargo doc --no-deps --open`).

## 1. Qué hace y qué no

- Pregunta una vez a la API de GitHub, o a la de GitHub Enterprise Server, cuál es la última versión estable publicada de un repositorio.
- Si es mayor que la local, devuelve un aviso con la versión y el enlace a su página.
- No descarga, no instala y no abre nada. Abrir el enlace lo decide el usuario del programa.
- No guarda estado ni caché: cada llamada hace una petición.
- Las versiones preliminares y los borradores no cuentan. La operación de la API usada (`releases/latest`) los excluye.

## 2. Cómo se incorpora

- La forma de incorporarla es única y obligatoria: `norma-de-integracion.md` (N1 a N4). Resumen: una copia en `bibliotecas/aviso-version-github/` y una dependencia por ruta:

```toml
[dependencies]
aviso-version-github = { path = "bibliotecas/aviso-version-github" }
```

- No está en crates.io (`publish = false`) ni se publica en GitHub. Cada programa lleva su propia copia.
- No añadas `ureq` por tu cuenta para el proxy: usa el reexportado `aviso_version_github::ureq`, para que las versiones coincidan.
- Qué programas la usan y con qué versión: el registro común, `registro de proyectos_bibliotecas.md`, en la carpeta que contiene el repositorio de la biblioteca. Un cambio aquí no les llega solo: cada programa vuelve a copiarlo y pasa sus puertas (apartado 11).

## 3. Uso mínimo

```rust
use aviso_version_github::{
    comprobar, proxy_del_sistema, AjustesDeVersionNueva, ConsultaHttps, OrigenDeLasVersiones,
};

fn buscar_version_nueva() {
    let ajustes = AjustesDeVersionNueva::para_repositorio("dueno/programa");
    let origen = match OrigenDeLasVersiones::desde_ajustes(&ajustes) {
        Ok(origen) => origen,
        Err(fallo) => return registrar(fallo),
    };
    let consulta = ConsultaHttps::nueva(&origen, proxy_del_sistema());
    match comprobar(env!("CARGO_PKG_VERSION"), &origen, &consulta) {
        Ok(Some(aviso)) => mostrar(&aviso.version, &aviso.enlace),
        Ok(None) => {} // Ya es la última versión.
        Err(fallo) => registrar(fallo),
    }
}
```

Este es el mínimo para entender la API. En un programa real se usa el módulo `aviso_de_version` de `examples/integracion_de_referencia.rs`, con hilo de trabajo, registro de fallos y `para_programa` (norma N3 a N7).

Orden obligatorio, sin atajos:
1. `AjustesDeVersionNueva`: con `para_repositorio("dueño/nombre")` o leídos de las preferencias con serde.
2. `OrigenDeLasVersiones::desde_ajustes(&ajustes)`: valida. Es la **única** forma de obtener un origen.
3. `ConsultaHttps::nueva(&origen, proxy_del_sistema())`, o `para_programa` (apartado 6).
4. `comprobar(version_local, &origen, &consulta)` o `ultima_version(&origen, &consulta)`.

Resultados de `comprobar`:
- `Ok(Some(aviso))`: hay una versión mayor. `aviso.version` es `X.Y.Z` sin `v`, y `aviso.enlace` es la página de esa versión.
- `Ok(None)`: la publicada es igual o menor que la local.
- `Err(FalloDeComprobacion)`: no se pudo saber. No es grave: no se avisa y, como mucho, se registra.

## 4. Reglas que no se saltan

- **Hilo de trabajo.** `comprobar` y `ultima_version` bloquean hasta que responde la red o se agota la espera (10 s por defecto). En un programa con interfaz, llámalas desde un hilo aparte y recoge el resultado por un canal.
- **Sin pánicos.** La biblioteca nunca hace `panic!`, `unwrap` ni `expect` fuera de sus pruebas, y lo vigila el compilador. No conviertas tú un `Err` en pánico: con `panic = "abort"` cerraría el programa entero.
- **Versión local X.Y.Z.** Pasa `env!("CARGO_PKG_VERSION")` o una cadena `X.Y.Z` (se admite una `v` delante). Una versión con sufijo, como `1.0.0-rc1`, da `Err(EtiquetaNoValida)`.
- **El enlace sale de los ajustes.** Nunca lo compongas tú a partir de la respuesta. La biblioteca ya lo construye con `url_web`, el repositorio y la etiqueta validada.
- **No edites la copia dentro del programa.** Los cambios se hacen en este repositorio y se vuelven a copiar.

## 5. Ajustes

`AjustesDeVersionNueva` se guarda y se lee con serde. Al leerlo, cada campo que falte toma su valor por defecto, salvo `repositorio`.

| Campo | Tipo | Por defecto | Validación en `desde_ajustes` |
|---|---|---|---|
| `repositorio` | `String` | ninguno (vacío, no valida) | `dueño/nombre`; letras, cifras, `-`, `_`, `.`; ninguna parte empieza por `.` |
| `url_api` | `String` | `https://api.github.com` (`URL_API_DE_GITHUB`) | solo `https://`, servidor válido, sin credenciales ni puerto, ruta limpia opcional |
| `url_web` | `String` | `https://github.com` (`URL_WEB_DE_GITHUB`) | igual que `url_api` |
| `segundos_de_espera` | `u64` | 10 (`SEGUNDOS_DE_ESPERA_POR_DEFECTO`) | distinto de cero |
| `limite_de_respuesta_en_bytes` | `u64` | 1048576 (`LIMITE_DE_RESPUESTA_POR_DEFECTO`) | distinto de cero |

Servidor válido: partes separadas por puntos, de 1 a 63 letras, cifras o guiones cada una, sin guion en los extremos y 253 caracteres como mucho en total. Se rechazan `.`, `a..b`, `-a.com` y `a.com.`. Una IP como `1.2.3.4` se acepta.

Ruta limpia: tramos de letras, cifras, `.`, `-` y `_`. Se rechazan los tramos vacíos, `.` y `..`, `?`, `#`, `%`, `\`, los espacios y los caracteres de control. Las barras finales se quitan.

Ejemplo de preferencias en JSON:

```json
{ "repositorio": "dueno/programa", "segundos_de_espera": 15 }
```

## 6. Identificar al programa en la cabecera User-Agent

GitHub pide que la cabecera User-Agent identifique a la aplicación. Por defecto la biblioteca envía `aviso-version-github/0.2.3`. Para presentar también al programa:

```rust
use aviso_version_github::{ConsultaHttps, ProgramaQueConsulta};

let programa = ProgramaQueConsulta::nuevo("mi-programa", env!("CARGO_PKG_VERSION"))?;
let consulta = ConsultaHttps::para_programa(&origen, proxy_del_sistema(), &programa);
// Cabecera: mi-programa/1.2.0 aviso-version-github/0.2.3
```

- Nombre: de 1 a 64 caracteres. El primero es letra o cifra ASCII y el resto letras, cifras, `.`, `-` o `_`. Si no cumple, devuelve `Err(IdentificacionNoValida)`.
- Versión: `X.Y.Z`, con `v` opcional. Si no cumple, devuelve `Err(EtiquetaNoValida)`.
- Usa un nombre fijo del programa. No metas en él datos del usuario ni del equipo.

## 7. Solo saber cuál es la última versión

```rust
use aviso_version_github::ultima_version;

let ultima = ultima_version(&origen, &consulta)?; // VersionSemantica
println!("Última versión: {ultima}");             // 1.4.2, sin «v»
```

- Devuelve la versión sin compararla con nada.
- `VersionSemantica` implementa `Ord`, así que se puede comparar con otra obtenida con `VersionSemantica::desde_etiqueta("1.2.0")`.
- Mismos fallos que `comprobar`.

## 8. Proxy y GitHub Enterprise Server

Proxy:
- `proxy_del_sistema()` mira primero `ALL_PROXY`, `HTTPS_PROXY` y `HTTP_PROXY`, en mayúsculas o minúsculas, con las excepciones de `NO_PROXY`. Si no hay ninguna y el programa corre en Windows, usa el proxy de las opciones de internet.
- No interpreta PAC ni WPAD ni se autentica con NTLM o Kerberos. En esos casos, el usuario define `HTTPS_PROXY` antes de abrir el programa.
- Proxy fijo: `Some(aviso_version_github::ureq::Proxy::new("http://proxy:3128")?)`. Sin proxy: `None`.

GitHub Enterprise Server:
- `url_api = "https://servidor/api/v3"` y `url_web = "https://servidor"`.

## 9. Fallos (`FalloDeComprobacion`)

| Variante | Cuándo |
|---|---|
| `Red(String)` | Sin conexión, tiempo agotado, certificado, estado HTTP de error (404 si el repositorio no tiene versiones publicadas, 403 por límite de peticiones), respuesta mayor que el límite |
| `RespuestaIlegible(String)` | El cuerpo no es JSON con `tag_name` de texto |
| `EtiquetaNoValida(String)` | La etiqueta publicada, la versión local o la del programa no es `X.Y.Z` |
| `AjustesNoValidos(String)` | `desde_ajustes` rechazó los ajustes |
| `IdentificacionNoValida(String)` | `ProgramaQueConsulta::nuevo` rechazó el nombre |

- Implementa `Display` (texto técnico para un registro) y `std::error::Error`.
- No muestres ese texto al usuario como si fuera grave: lo normal es no avisar y, como mucho, registrarlo.
- El límite de peticiones sin autenticar de GitHub es bajo. No llames en bucle: una comprobación al arrancar, o por acción del usuario, basta.

## 10. Pruebas en el programa que la usa

- No hagas peticiones reales en las pruebas. Implementa el rasgo `ConsultaDeVersion` con una consulta falsa:

```rust
use aviso_version_github::{ConsultaDeVersion, FalloDeComprobacion};

struct ConsultaFalsa(&'static str);

impl ConsultaDeVersion for ConsultaFalsa {
    fn pedir(&self, _url: &str) -> Result<String, FalloDeComprobacion> {
        Ok(format!(r#"{{"tag_name":"{}"}}"#, self.0))
    }
}
// comprobar("1.0.0", &origen, &ConsultaFalsa("v1.1.0")) devuelve Ok(Some(aviso)).
```

- El rasgo exige `Send`.
- Las funciones puras `interpretar_respuesta`, `decidir_aviso` y `VersionSemantica::desde_etiqueta` se pueden probar sin consulta.

## 11. Pasar de la 0.1.0 a la 0.2.0

La 0.2.0 rompe la compatibilidad en dos puntos:
1. `ConsultaHttps::nueva` recibe `&OrigenDeLasVersiones` en vez de `&AjustesDeVersionNueva`. Cambia `ConsultaHttps::nueva(&ajustes, proxy)` por `ConsultaHttps::nueva(&origen, proxy)`, con el origen que ya obtenías con `desde_ajustes`.
2. `FalloDeComprobacion` tiene la variante nueva `IdentificacionNoValida`. Un `match` exhaustivo sobre el fallo necesita su rama o un comodín.

Novedades que no rompen nada: `ultima_version`, `ProgramaQueConsulta` con `ConsultaHttps::para_programa`, y la validación más estricta del nombre del servidor. Unas preferencias con un servidor como `a..b` que antes se aceptaban ahora dan `AjustesNoValidos`.

Al actualizar la copia de un programa:
1. Sustituye su carpeta `bibliotecas/aviso-version-github/` por la versión nueva, sin `target/`.
2. Adapta las llamadas según los puntos de arriba.
3. Pasa las puertas del programa en Linux y Windows: `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check` y `cargo audit`.
4. Si el programa tiene su propio `Cargo.lock`, revisa que solo cambian las dependencias de esta biblioteca.
5. Pon al día el apartado del programa en el registro común y la documentación del programa que hable del aviso de versión.
