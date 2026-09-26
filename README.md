# Programator

Programator es una aplicación portable escrita en Rust para Windows que convierte un modelo de inteligencia artificial local, ejecutándose en la tarjeta gráfica del propio equipo, en un colaborador activo dentro del equipo de trabajo.

El agente se integra en el flujo del proyecto recibiendo encargos de desarrollo y entregando propuestas de código mediante el intercambio de archivos de texto en un canal compartido, sin enviar datos a servicios externos y con total independencia de conexiones remotas para la inferencia.

## Principio rector de diseño

Todo el código y la arquitectura de Programator responden a una premisa de seguridad estricta: **el modelo de lenguaje nunca escribe directamente en el proyecto**.

El modelo no tiene permisos de edición sobre los archivos de trabajo ni acceso directo al sistema operativo. Para cualquier operación, solicita una acción formal a través de las herramientas proporcionadas por el arnés, y es este quien evalúa y concede o deniega cada petición de forma controlada. Las propuestas de código se escriben de manera aislada en un directorio de candidatos para que otro agente o un revisor humano las inspeccione, valide y traslade al código fuente definitivo.

## Para qué sirve

Programator tiene sentido cuando trabajas con un modelo de pago y quieres que la parte más mecánica del trabajo la haga otro. Esa es la idea de la que nace, y de ahí salen cuatro usos que se han dado de verdad, no imaginados.

El primero es quitarle trabajo al modelo de pago. Hay tareas que un modelo local hace con garantías: auditar un fichero cargado de números sueltos y métodos largos, escribir una función acotada cuando le das los requisitos en una lista, traducir código siguiendo un patrón que ya existe. Encargarle eso libera cuota del modelo caro para lo que de verdad la necesita. Pero el ahorro de cuota no es lo que más se nota: lo que más se nota es que **trabaja en paralelo**. Mientras el modelo local dedica sus dos o tres minutos a una auditoría, quien lo dirige sigue con lo suyo. Al final de una jornada eso son varias tareas hechas que no han costado ni una espera.

El segundo es medir al propio modelo local. Un modelo no es bueno o malo en abstracto: es bueno en unas tareas y malo en otras, y la única forma de saber en cuáles es probarlo con encargos reales y verificar lo que entrega. Programator sirve para eso, y no es una suposición: la carpeta `evaluacion` de este repositorio contiene el banco de pruebas con el que se midió: el plan y la rúbrica de los diez encargos, dos scripts de barrido que traen su encargo escrito dentro, semilla fija para que los resultados no dependan del azar, y la configuración exacta que se usó. De ahí salieron las cifras que hoy usamos para decidir qué encargarle. Ese mismo banco sirve para lo otro que hay que medir, que es la configuración: temperatura, tamaño de contexto, cuántas capas caben en la tarjeta. Cambiar un parámetro y ver qué pasa deja de ser intuición y pasa a ser una medida.

El tercero es que tu código no sale de tu máquina. La inferencia ocurre entera en tu propia tarjeta gráfica: el modelo local lee tus ficheros, escribe sus propuestas y no manda una sola línea a ningún servicio. Para trabajo bajo acuerdo de confidencialidad, o simplemente si prefieres que tu código no viaje, eso puede ser la diferencia entre poder usar una herramienta así o no poder.

Conviene aclarar qué significa exactamente, porque es fácil entenderlo de más. Que la inferencia sea local no quiere decir que puedas trabajar sin conexión. Programator no tiene una consola donde tú escribas: recibe los encargos leyendo ficheros de texto del canal, y quien normalmente los escribe es otro agente, que sí es un modelo de pago y sí necesita internet. Así que en el uso habitual hace falta conexión, igual que antes.

Lo que se queda en casa es tu código y lo que el modelo local hace con él, que no es poco. Y si alguna vez quisieras prescindir del agente que dirige, nada impide que escribas tú mismo el encargo a mano en el buzón del canal con un editor de texto, porque el arnés solo busca un encabezado dentro de un fichero. Pero eso no está documentado ni se ha probado, así que no lo cuentes como una forma de trabajo probada: es una posibilidad, no una promesa.

El cuarto es probar otros modelos. Programator no está casado con el modelo con el que se ha desarrollado. Si quieres saber qué te daría otro modelo local, puedes cambiarlo en la configuración y pasarle el mismo banco de pruebas, con los mismos encargos y la misma semilla, y comparar resultados que significan algo porque las condiciones fueron idénticas. Visto así, esto es también un banco de pruebas para decidir con qué modelo quedarse.

## Para qué no sirve

Contar solo lo que hace bien sería venderlo por encima de lo que da, así que aquí va la otra mitad. Todo lo que sigue está medido, y el detalle está en `evaluacion/desempeno-uso-real-natureland.md`.

No le pidas que diagnostique la causa de un fallo. Es lo que peor hace con diferencia: en la prueba real acertó cero de doce, y las doce explicaciones sonaban plausibles. Si necesitas saber por qué algo se comporta como se comporta, mide tú; un modelo local no sustituye una medición.

No le preguntes por bibliotecas, versiones ni parámetros de una interfaz de programación. Se las inventa, y lo hace justo donde más daño causa: no en las preguntas raras, sino en las razonables, donde lo inventado suena a algo que debería existir. Además no puede comprobarlo, porque no navega ni consulta documentación.

No le des un fichero que crees limpio para que te lo confirme. Ante un fichero cuidado no dice «no veo nada»: rellena con hallazgos inventados. Su acierto sube cuando el fichero de verdad tiene defectos, y se desploma cuando no los tiene.

No esperes que apruebe, firme ni acepte nada. No es una limitación temporal, es el principio de diseño: el modelo no dispone de ese verbo y no va a disponer de él.

Y verifica siempre lo que entregue. No por desconfianza, sino porque verificarlo es barato y equivocarse no lo es.

## Sobre el equipo en el que se ha probado

Conviene que sepas en qué máquina se ha desarrollado y medido todo esto, porque las cifras de arriba salen de ahí y en otro equipo pueden ser distintas.

La tarjeta gráfica es una NVIDIA GeForce RTX 4070 Ti SUPER con 16 GiB de memoria, de los que el sistema ve 16.376 MiB. El modelo local es `devstral-small-2-24b-Q4_K_M.gguf`, de 40 capas y 13,3 GiB. El motor de inferencia es `llama-server`, compilación b10993 con CUDA 12.4. La ventana de contexto por defecto son 16.384 tokens.

Con esa configuración, Programator coloca 39 de las 40 capas del modelo en la tarjeta y deja un margen de seguridad de 1 GiB, lo que da unos 18,4 tokens por segundo. Forzando las 40 capas la generación sube a 23,0 tokens por segundo, pero entonces el proceso ocupa 15.851 de los 16.376 MiB disponibles y el margen se vuelve muy estrecho. Por eso el valor por defecto es el conservador.

Nada de esto significa que la aplicación solo funcione ahí. Programator mide la tarjeta que encuentre, pesa el modelo que le indiques y calcula cuántas capas caben, así que se adapta a otro equipo sin que tengas que tocar nada. Lo que sí cambia con el hardware es el rendimiento, y si tu tarjeta tiene menos memoria es posible que tengas que usar un modelo más pequeño. Las cifras de esta sección son un punto de referencia de un equipo concreto, no una promesa sobre el tuyo.

## Puesta en marcha

La puesta en marcha detallada se explica en la guía `plantillas/LEEME-portable.md`. Allí encontrarás los tres pasos para iniciar el arnés, la preparación de dependencias locales (el ejecutable `llama-server` con soporte CUDA y el archivo GGUF del modelo) y la resolución de incidencias frecuentes en el arranque.

## Compilación y verificación mecánica

El proyecto se rige por un criterio de verificación estricto: ninguna entrega se acepta sin la superación limpia e independiente de tres comprobaciones mecánicas:

```
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

Las tres órdenes deben ejecutarse sin fallos, sin advertencias y sin diferencias de formato.

## Estructura del código fuente (src)

El código fuente se organiza en módulos con responsabilidades aisladas:

- `src/main.rs`: Punto de entrada del binario, inicialización del bucle principal de sondeo y gestión de paradas.
- `src/config.rs`: Carga, validación tipada y valores por defecto del archivo de configuración TOML.
- `src/carpeta.rs`: Resolución, validación y persistencia de la carpeta de trabajo del proyecto.
- `src/protocolo/`: Implementación del canal de comunicación basado en archivos (lectura de buzones, emisión de latido, gestión de estado y propuestas de poda).
- `src/motor/`: Interacción con `llama-server`, control del ciclo de vida del proceso de inferencia, evaluación de salud del servidor y cálculo de capas en GPU.
- `src/arnes/`: Lógica central del sistema, incluyendo la ficha del repertorio, composición de instrucciones, ciclo de inferencia, colapso de repeticiones y verificación de sintaxis de entregas.
- `src/arranque/`: Funciones de inicio de sesión, publicación de guías de trabajo, instalación de normas y avisos de disponibilidad en consola.

## Guía de documentación (docs)

El directorio `docs/` contiene lo que hace falta saber para usar el programa y para seguir trabajando en él:

- `docs/defectos-conocidos.md`: Documento de obligada consulta que detalla las limitaciones vigentes, las pruebas que requieren hardware real y los aspectos estructurales pospuestos.
- `docs/pruebas-manuales.md`: Procedimientos de verificación sobre tarjeta gráfica y modelo real. Ninguna prueba automática enciende la tarjeta, así que esta es la única comprobación de la junta entre el arnés y el motor.
- `docs/hoja-de-ruta.md`: Qué trae cada versión y qué está previsto a continuación, con el criterio que ordena las prioridades.
- `docs/decisiones/`: Decisiones de arquitectura en vigor, con las alternativas que se descartaron y por qué.

## Sobre Celtilander

Soy un usuario novel en el mundo de la programación y la IA. He «ayudado a crear» esta app y espero
que a alguien le pueda servir para algo... sin más.

Lo que no funciona no lo escondo: está escrito en defectos conocidos. Si encuentras algo que no esté
ahí, cuéntamelo.

Para realizar esta app he montado un sistema colaborativo y de consenso entre ChatGPT Codex, Claude
Code y Gemini Antigravity. Yo puse la idea y dirigía el proyecto, mientras que ellos diseñaban,
repartían y verificaban el trabajo. Ningún código fue revisado por el agente que lo había escrito, y
una vez terminado todo debía ser aprobado por unanimidad. Hasta que no se conseguía el consenso, no
se daba una tarea por concluida. Cuando por algún motivo no se llegaba a un consenso, era yo quien
tomaba la decisión final, así que los fallos que pueda haber serán más míos que de ellos.

Saludos a todo el mundo.

## Licencia

**GPL-3.0 o posterior.** El texto íntegro está en [`LICENSE`](LICENSE).

En corto: puedes usar el programa para lo que quieras, estudiar cómo funciona por dentro,
modificarlo y repartirlo. La única condición es que **si repartes una versión modificada, publiques
también su código**, con esta misma licencia.

Se eligió copyleft y no una licencia permisiva a propósito, y el motivo es el mismo principio que
gobierna el código. Programator existe para que una persona conserve el control sobre lo que hace un
modelo: por eso el modelo pide permiso y el arnés concede o deniega, en lugar de escribir por su
cuenta. Una herramienta que defiende esa idea no debería poder cerrarse y venderse sin devolver
nada. Lo que se construya encima vuelve a todos.
