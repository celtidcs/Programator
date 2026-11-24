# Problemas frecuentes

Síntoma, causa y qué hacer. El texto del fallo es el `Display` de `FalloDeComprobacion`, tal como queda en el registro del programa.

## Nunca avisa

- ¿Está `comprobar_version_nueva` a `false` en las preferencias? Entonces no se consulta nada (N5.1).
- ¿Hay un fallo registrado? Búscalo en el registro del programa, con los síntomas de abajo.
- ¿La última versión publicada es una preliminar (`-rc1`) o un borrador? La API la excluye: `releases/latest` solo devuelve versiones estables.
- ¿Las etiquetas del repositorio no son `X.Y.Z` o `vX.Y.Z`? Una etiqueta como `version-1.2` da `etiqueta de versión no válida`. Publica con `vX.Y.Z`.
- ¿La versión local es mayor o igual que la publicada? Es lo correcto: no hay nada que avisar.

## «fallo de red: ... 404»

- El repositorio no existe, es privado o no tiene ninguna versión publicada (solo etiquetas sin release).
- Revisa `ajustes_de_version.repositorio` y que en GitHub haya una release publicada, no un borrador.

## «fallo de red: ... 403»

- Casi siempre es el límite de peticiones sin autenticar de GitHub, que se cuenta por dirección IP.
- No reintentes en bucle (N6.1). Basta una comprobación al arrancar: la siguiente vez funcionará.

## «fallo de red: ...» con tiempo agotado, DNS o conexión rechazada

- Sin red, o un proxy que la biblioteca no ve.
- La biblioteca usa `HTTPS_PROXY`, `HTTP_PROXY` o `ALL_PROXY` (con `NO_PROXY`) y, en Windows, el proxy de las opciones de internet.
- No interpreta PAC ni WPAD ni se autentica con NTLM o Kerberos. En esas redes, el usuario define `HTTPS_PROXY=http://proxy:puerto` antes de abrir el programa.
- Si la red es lenta, sube `ajustes_de_version.segundos_de_espera` en las preferencias, nunca en el código (N4.3).

## «fallo de red: ...» con certificado

- La biblioteca solo confía en las raíces públicas de webpki-roots, no en el almacén del sistema.
- Un proxy que intercepta TLS con un certificado propio de la empresa no funcionará. `ConsultaHttps` no ofrece forma de añadir raíces.
- En GitHub Enterprise Server con certificado propio pasa lo mismo: tiene que tener un certificado emitido por una autoridad pública.

## «ajustes de comprobación no válidos: ...»

- `desde_ajustes` rechazó las preferencias. El texto dice qué campo.
- Repositorio: tiene que ser `dueño/nombre`.
- Direcciones: solo `https://`, sin usuario ni puerto, con un nombre de servidor válido (`a..b` o `-a.com` no valen) y una ruta limpia.
- `segundos_de_espera` y `limite_de_respuesta_en_bytes` no pueden ser 0.

## «respuesta ilegible: ...»

- El servidor no devolvió el JSON de una release. Pasa con un portal cautivo (wifi de hotel), con un proxy que devuelve su propia página o con `url_api` apuntando a la web y no a la API.
- En GitHub Enterprise Server, `url_api` es `https://servidor/api/v3`, no `https://servidor`.

## «nombre de programa no válido: ...»

- `NOMBRE_DEL_PROGRAMA` tiene espacios, barras u otros caracteres no admitidos.
- Usa minúsculas, cifras y guiones, como `mi-programa`. La prueba de N10 lo detecta antes de compilar la entrega.

## El programa se congela un momento al arrancar

- Se está llamando a `comprobar` en el hilo de la interfaz, o se espera con `recv` en él.
- Usa el `arrancar` de la referencia y `try_recv` (N6).

## Error de compilación al actualizar a la 0.2.0

- `expected &OrigenDeLasVersiones, found &AjustesDeVersionNueva` en `ConsultaHttps::nueva`: pásale el origen validado (`guia-de-uso-para-agentes.md`, apartado 11).
- `non-exhaustive patterns: IdentificacionNoValida` en un `match` sobre `FalloDeComprobacion`: añade la rama o un comodín.

## Las pruebas del programa salen a la red

- Alguna prueba construye `ConsultaHttps`. Las pruebas usan siempre una consulta falsa que implementa `ConsultaDeVersion` (N10).

## Ninguna de estas causas

- Anota el fallo en tu apartado del registro común, `registro de proyectos_bibliotecas.md`, con cómo reproducirlo. No lo arregles en la copia de tu programa (N1.3).
