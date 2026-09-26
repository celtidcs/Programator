# Evaluación del candidato — Devstral Small 2 24B sobre Programator

> **Qué es esto.** Una evaluación técnica de un modelo local como si fuera un candidato a
> incorporarse al equipo de desarrollo. Las órdenes se le dan **a través de Programator**, por el
> canal de ficheros, exactamente como se le darían en producción: no se le habla directamente al
> motor. Así se evalúan dos cosas a la vez, el candidato y la herramienta que lo integra.
>
> Sesión del 22/09/2026. Las órdenes se dieron por el canal y las entregas se compilaron y probaron
> antes de puntuarlas, sin dar por bueno nada por lo que el candidato dijera de sí mismo.

## Montaje

| Pieza | Qué se usó |
|---|---|
| Arnés | Programator 0.5.0, recompilado desde `main` con dos arreglos de esta sesión |
| Modelo | `devstral-small-2-24b-Q4_K_M.gguf`, 40 capas, 13,3 GiB |
| Motor | `llama-server` b10993 con CUDA 12.4 |
| Tarjeta | NVIDIA GeForce RTX 4070 Ti SUPER, 16 GiB |
| Encaje | 33-35 capas en GPU según la VRAM libre; el resto por CPU |
| Muestreo | temperatura 0,2 · semilla fija 42 · contexto 32768 |

La semilla es fija **a propósito**: si el candidato acierta o falla, que no sea por el azar del
muestreo. La configuración exacta está en `programator-evaluacion.toml`.

## Cómo se puntúa

Cada encargo se puntúa de **0 a 5**, y la nota no sale de leer la respuesta por encima: **el código
se compila y se ejecuta**, y se le pasan pruebas que el candidato no ha visto. Un código que
compila y pasa sus propias pruebas es un aprobado raso; lo que separa a un junior de un senior es
lo que ocurre con las pruebas adversarias.

| Nota | Significado |
|---|---|
| 5 | Lo entregaría a producción tal cual |
| 4 | Correcto; retoques menores en revisión |
| 3 | Sirve, pero necesita una revisión seria antes de entrar |
| 2 | Tiene la idea, el resultado no se sostiene |
| 1 | Responde pero no resuelve |
| 0 | No entrega, o entrega algo falso |

Ejes que se miran en cada entrega:

1. **Corrección** — ¿hace lo que se pidió? ¿Compila? ¿Pasa pruebas que no escribió él?
2. **Seguimiento de instrucciones** — los encargos llevan requisitos explícitos y prohibiciones.
   Ignorar uno cuenta, aunque el código funcione.
3. **Robustez** — entrada rara, límites, recursos. Lo que separa un juguete de un servicio.
4. **Idiomaticidad** — ¿escribe como se escribe en ese lenguaje, o traduce de otro?
5. **Honestidad** — ¿dice lo que ha hecho de verdad, o adorna? ¿Admite lo que no sabe?

## La batería

| Id | Qué mide | Lenguaje |
|---|---|---|
| E01 | Protocolo y autoconocimiento: qué herramientas tiene y qué no puede hacer | — |
| E02 | Diseño de aplicaciones: arquitectura, conflictos, supuestos | — |
| E03 | Código de sistemas: errores tipados, sin pánico, pruebas | Rust |
| E04 | Tipos: uniones discriminadas, validación, sin `any` | TypeScript |
| E05 | Código idiomático y casos borde | Python |
| E06 | Asincronía y recursos | C# |
| E07 | Metatablas y ámbito | Lua |
| E08 | Consultas no triviales e índices | SQL |
| E09 | Depuración: encontrar un fallo ajeno | (el que toque) |
| E10 | Honestidad: qué hace ante algo que no existe | — |

SQL entra en la lista aunque no se pidiera: un candidato que escribe servicios y no sabe leer un
plan de consulta es un riesgo, y en este equipo se toca base de datos.

## Estructura de la carpeta

```
evaluacion/
├── README.md                     este documento: plan y rúbrica
├── programator-evaluacion.toml   la configuración exacta con la que se midió
├── banco-de-principios.py        experimento A/B sobre las instrucciones de sistema
├── banco-de-parametros.py        barrido de temperatura, contexto y capas
├── informe-final.md              la evaluación del banco y el veredicto
├── rendimiento.md                las cifras de velocidad medidas
└── desempeno-uso-real-natureland.md   cómo se comportó en un proyecto de verdad
```

**Lo que no está aquí, y por qué.** El material en bruto de aquella sesión —las respuestas tal cual
las escribió el candidato, los proyectos donde se compiló cada entrega y el registro de fallos del
arnés que se fue tomando en caliente— se quedó en el repositorio de trabajo. Ocupa mucho, solo tiene
sentido junto a la máquina en la que se midió y no aporta nada que no esté ya contado aquí con sus
cifras. Lo que sí se publica es todo lo necesario para repetir la medida: el plan, la rúbrica, la
configuración exacta y los dos scripts que hacen el barrido.

Se rescató una sola pieza de aquel material, `banco/.gestor/candidatos/programator/reintentos.py`,
porque no es documentación: es la entrega real contra la que se verifica el comprobador en
`tests/puerta_contra_las_entregas_reales.rs`. Sin ella, quien clone el repositorio se encontraría la
suite en rojo.

## Y después del banco: cómo se ha comportado en uso real

El banco de arriba mide **capacidad**, con encargos preparados y pruebas adversarias. Lo que hace en
un proyecto de verdad es otra pregunta, y está medida aparte:

**[`desempeno-uso-real-natureland.md`](desempeno-uso-real-natureland.md)** — 14 encargos sobre
NatureLand (Godot + C#) entre el 22 y el 24 de septiembre de 2026, todos verificados contra el
código. Incluye el balance por tipo de tarea, los cinco modos de fallo con su antídoto, una tabla de
decisión sobre qué encargarle, y —lo más útil para seguir desarrollando el arnés— **seis cambios
concretos al propio Programator deducidos de fallos observados** (§9).
```
