# Base de conocimiento de aviso-version-github

Punto de entrada para cualquier agente que tenga que usar, conectar, actualizar o revisar esta biblioteca en un programa. Describe la versión 0.2.3.

## Qué leer según lo que vayas a hacer

| Vas a... | Lee entero |
|---|---|
| Integrar la biblioteca en un programa por primera vez | `norma-de-integracion.md` y `../examples/integracion_de_referencia.rs` |
| Actualizar la copia de un programa a una versión nueva | `norma-de-integracion.md` (N11) y `guia-de-uso-para-agentes.md` (apartado 11) |
| Saber qué hace una función, un ajuste o un fallo | `guia-de-uso-para-agentes.md` |
| Entender por qué no avisa o qué significa un fallo | `problemas-frecuentes.md` |
| Saber qué programas la usan y con qué versión, o anotar un fallo de la biblioteca | `registro de proyectos_bibliotecas.md`, el registro común (ver abajo) |
| Verificar la integración de otro agente | La lista de comprobación al final de `norma-de-integracion.md` |
| Cambiar la propia biblioteca | `../README.md`, el código de `../src/` y `../mejoras.md` |

## Documentos

- `norma-de-integracion.md`: **obligatoria**. La forma única de conectar la biblioteca (N1 a N11) y la lista de comprobación para aceptar una integración.
- `../examples/integracion_de_referencia.rs`: la norma hecha código. El módulo `aviso_de_version` se copia tal cual en el programa. Cargo lo compila y `cargo test` ejecuta sus pruebas, así que siempre está al día con la biblioteca.
- `guia-de-uso-para-agentes.md`: referencia de la API, con ajustes, fallos, proxy, GitHub Enterprise Server, pruebas sin red y la migración de la 0.1.0 a la 0.2.0.
- `problemas-frecuentes.md`: síntoma, causa y solución de los fallos habituales.
- `registro de proyectos_bibliotecas.md`: el registro común. Está fuera de este repositorio, en la carpeta que lo contiene, no va con las copias y es común a todas las bibliotecas. Cada programa se da de alta en él, anota con qué versión de la copia trabaja y sus desviaciones de la norma, y deja los fallos que encuentre en la biblioteca. Quien mantiene la biblioteca lo lee al empezar cada sesión.

## Reglas que no se olvidan

- Una copia por programa, en `bibliotecas/aviso-version-github/`, idéntica a una etiqueta de este repositorio. No se edita allí.
- Un solo módulo `aviso_de_version` por programa.
- La comprobación va en un hilo de trabajo, una vez al arrancar.
- Los fallos se registran: ni se callan ni se muestran como errores.
- Las pruebas usan una consulta falsa y no salen a la red.
- Después de integrar o actualizar, se pone al día el apartado del programa en el registro común y la documentación del programa.
- Un fallo encontrado en la biblioteca se anota en ese registro, no se arregla en la copia.
