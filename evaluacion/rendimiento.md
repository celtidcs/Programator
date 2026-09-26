# Rendimiento del motor: qué cuesta la configuración actual

> Medido el 22/09/2026 sobre la máquina del Director, tras la evaluación del candidato.
> Modelo: `devstral-small-2-24b-Q4_K_M`, 40 capas, 13,3 GiB. Tarjeta: RTX 4070 Ti SUPER, 16 GiB.
> Misma petición en las tres pruebas, con `temperature 0.2`, `seed 42` y `max_tokens 400`.

## La medida

| Configuración | Capas en GPU | Contexto | Slots | VRAM | Generación |
|---|---|---|---|---|---:|
| **La que usa Programator 0.5.0** | 35/40 | 32768 | 4 | ~15,8 GiB | **9,8 t/s** |
| 40 capas, contexto completo | 40/40 | 32768 | 1 | 15,8 GiB | 15,4 t/s |
| **40 capas, contexto 16k** | 40/40 | 16384 | 1 | 15,8 GiB | **23,0 t/s** |

**2,35 veces más rápido**, sin tocar el modelo ni el hardware.

## De dónde sale la pérdida

El log del arranque lo dice en su primera línea:

```
srv load_model: initializing, n_slots = 4, n_ctx_slot = 32768, kv_unified = 'true'
```

`llama-server` reserva por defecto caché para **cuatro conversaciones simultáneas**. Programator
atiende **una cada vez**: `ejecutar_pasada` recorre los encargos en serie y nunca lanza dos
peticiones a la vez. Esa reserva cuádruple es VRAM que no se usa nunca, y es justo la que falta
para meter las cinco últimas capas del modelo en la tarjeta.

Las capas que se quedan en CPU no cuestan «un poco»: en esta medida cuestan **más de la mitad del
rendimiento**. Pasar de 35/40 a 40/40 con un solo slot más que duplica la velocidad.

El contexto también se paga. Con 32768 reservados el modelo entero cabe, pero deja el margen tan
justo que la generación baja a 15,4 t/s; a 16384 sube a 23,0. Las normas de la casa ocupan unos
8.000 tokens del contexto, así que 16k deja sitio de sobra para el trabajo de un encargo.

## Qué hacer con esto

Los tres parámetros —`--parallel`, `--ctx-size` y las capas— **están hoy incrustados en el código**
(`src/motor/proceso.rs`), y `--parallel` ni siquiera se pasa: se acepta el valor por defecto del
motor. Sacarlos al TOML es exactamente la 0.7.0 ya planificada; esta medida le pone número al
beneficio.

Recomendación concreta para esa versión:

1. **Pasar `--parallel 1` explícitamente.** Programator es de un solo hilo de trabajo por
   definición; aceptar el valor por defecto del motor regala tres cuartas partes de la caché.
2. **Contexto configurable, con 16384 por defecto** en vez de 32768. Quien necesite más, que lo
   suba sabiendo lo que cuesta.
3. **Anotar en el informe de arranque cuánta VRAM se lleva la caché**, no solo cuántas capas caben.
   Hoy el encaje decide capas sin contar lo que el propio motor va a reservar por su cuenta, y esa
   es la razón de que el cálculo diga 35 y la realidad permita 40.

El punto 3 es el más de fondo: el cálculo del encaje de la 0.4.0 pesa el modelo y mide la VRAM
libre, pero **no sabe lo que `llama-server` reservará para la caché**. Por eso acierta en la
aritmética y se queda corto en la práctica.

---

## ¿Y si el caché va en RAM? Medido el 22/09/2026

La pregunta del Director: si la VRAM es el cuello, **¿no se puede poner el contexto en la RAM del
sistema y dejar la tarjeta para el modelo?** `llama.cpp` lo permite con `--no-kv-offload`.

Se midió con la misma petición y la misma semilla, primero con el contexto casi vacío y después
con **13.000 tokens dentro**, que es lo que de verdad distingue las dos configuraciones:

| Caché | Contexto | Capas en GPU | VRAM | Contexto casi vacío | **Contexto lleno (13k)** |
|---|---|---|---|---|---:|
| **En RAM** (`--no-kv-offload`) | 32768 | 40/40 | 14,9 GiB | 21,3 t/s | **6,3 t/s** |
| **En VRAM** | 16384 | 39/40 | 15,8 GiB | 23,0 t/s | **17,0 t/s** |

**Con el contexto vacío parece un chollo** —32k de ventana, las cuarenta capas dentro y 21 t/s—,
y con el contexto lleno **se hunde: 2,7 veces más lento**.

El motivo es estructural y no se arregla afinando: el caché se lee y se escribe **en cada token que
se genera**, así que cruzar el bus PCIe se paga en cada paso, y el precio sube conforme el contexto
se llena. El prompt sí va rápido en los dos casos (1.250 contra 1.495 tokens por segundo), porque
esa parte se procesa en paralelo; lo que se degrada es la generación, que es secuencial.

**Conclusión: el caché se queda en VRAM.** Y la pregunta de fondo tiene mejor respuesta que ampliar
el contexto.

### Dónde está el contexto de verdad

El Director trabaja **fichero a fichero**, no pidiendo proyectos enteros de una vez. Para eso 16k
sobran… salvo por un detalle que se come la mitad:

> **Las normas de la casa ocupan unos 8.000 tokens de los 16.384, en cada encargo.**

Son 30 KB de protocolo que el modelo relee entero cada vez. Con un preámbulo de unos 1.000 tokens
—lo que le aplica a Programator, no el documento completo del equipo— quedarían unos 15k libres:
**un fichero de 700 a 1.000 líneas, con sitio de sobra para la respuesta**.

Y hay un segundo beneficio, menos obvio: en la evaluación, el candidato **ignoró requisitos
explícitos del encargo en cuatro de las diez pruebas**. Ocho mil tokens de normas genéricas
compitiendo por su atención con el encargo no ayudan a que los cumpla.

**Recomendación:** dejar el caché en VRAM, mantener el contexto en 16384, y **recortar el preámbulo**.
Es más rápido, más barato y probablemente mejore el seguimiento de instrucciones. `--no-kv-offload`
queda como herramienta puntual para el día que haga falta una ventana enorme, sabiendo que la
generación irá a un tercio.
