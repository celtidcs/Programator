# La revisión mensual de modelos

Este es el procedimiento que sigue el modelo de pago (Claude), una vez al mes, para buscar modelos
locales mejores que los que hay, medirlos con el banco y quedarse con los que ganen. **Lo hacéis
entre Programator y tú, sin pedirle nada al Director**: él abre los dos sobre la carpeta de
Programator, te pide la revisión y al final lee tu informe. Lo ordenó el 13/11/2026: las decisiones
salen de las reglas de puntuación, no de su permiso.

## Quién hace qué

- **Programator** hace lo mecánico: busca en Hugging Face, descarga, suelta su motor para pasar el
  banco, comprueba las tandas, anota el registro, aplica la regla, retira y cambia el modelo por
  defecto. Lo hace su código, no su modelo local, que no ve nada de esto.
- **Tú** pones el juicio: qué candidatos se prueban, las notas de las rúbricas y el informe.

Se lo pides por el canal, con encargos en tu buzón de `.gestor/canal/` dirigidos a Programator. La
primera línea tras «Para Programator:» es el pedido; Programator contesta en su buzón con lo que hizo y con el
paso siguiente. Tu buzón tiene que estar en `[revision] buzones_que_ordenan` de
`programator.toml`; si no, Programator contesta que no puedes pedirlo.

```
Para Programator:
revisión: buscar
```

## 1. Buscar

Pide `revisión: buscar`. Programator deja en `revisiones/AAAA-MM/`:

- `candidatos.md`, para leer: los candidatos con su tamaño, licencia, fecha y descargas, y los
  descartados agrupados por motivo;
- `candidatos.toml`, con los nombres exactos que se pueden pedir;
- `respuestas/`, con lo que contestó Hugging Face.

## 2. Elegir qué se prueba

Lee `candidatos.md` y elige como mucho `[revision] candidatos_por_revision` (por defecto 3). Cada
uno son unas dos horas de GPU, así que elige con criterio:

- uno por repositorio, en la cuantización que mejor quepa (Q4_K_M o Q5_K_M antes que Q8_0);
- antes un modelo distinto de los que ya hay que otra variante del mismo;
- fuera lo que `llama-server` no carga, lo que la licencia no deja usar, lo que sube alguien de
  poca confianza y los modelos modificados para quitarles salvaguardas;
- si dudas de uno, déjalo para la siguiente revisión: no se pierde, la búsqueda lo vuelve a dar.

Puedes buscar en internet para decidir. Anota por qué eliges cada uno: va en el informe.

## 3. Probar

Pídelos, uno por línea con el repositorio y el fichero exactos de `candidatos.toml`:

```
Para Programator:
revisión: probar
ManniX-ITA/JackOD-9B-Coder-MTP-GGUF JackOD-9B-Coder-CD-Q4_K_M.gguf
```

Por cada uno, Programator lo descarga (o lo encuentra ya descargado, con su huella comprobada),
suelta su motor, pasa el banco y comprueba la tanda en este mismo equipo, con el
`[comprobacion_local]` de `banco/banco.toml`; sin esa sección, las notas de oficio salen «no
evaluado». Si la comprobación falla, las pasadas quedan guardadas y se comprueban al cerrar.
Mientras tanto, su latido dice qué candidato está probando. Su motor vuelve a arrancar con el siguiente encargo normal.

Al terminar publica cómo quedó cada candidato y la lista de rúbricas que te toca puntuar.

## 4. Puntuar las rúbricas

En cada tanda, `banco/resultados/<tanda>/rubricas/`. Lee cada respuesta entera antes de poner la
nota, escríbela en su línea `Nota:` y deja los motivos en `rubricas/motivos-de-las-notas.md` de esa
tanda. No inventes una nota para cerrar antes: una rúbrica sin nota detiene el cierre, y es lo
correcto.

Algunas fichas traen código en vez de una respuesta: son encargos de código cuyas pruebas ocultas
no dieron nota (apagadas, o sin poder ejecutarse). Puntúalas **leyendo el código, sin ejecutarlo**,
con la rúbrica que llevan debajo. Esa nota no es comparable con la de las pruebas, y Programator
lo tiene en cuenta al cerrar (punto 5): no hace falta que compares tú.

## 5. Cerrar

Pide `revisión: cerrar`. Programator:

1. vuelve a intentar una vez las tandas que quedaron sin comprobar;
2. si queda alguna rúbrica sin nota, no hace nada más y te dice cuáles faltan;
3. rehace los informes de las tandas y las anota en el registro;
4. aplica la regla: en cada una de las nueve características (las seis clases de encargo y las
   tres familias) gana un modelo, con el desempate de siempre (inventados, velocidad, tamaño). **Se
   queda todo el que gane alguna; se retira el que no gane ninguna**: se anota en el registro con
   sus cifras y su motivo, y se borra su fichero;
5. si el modelo por defecto (`[motor] modelo`) no gana ninguna y hay quien lo sustituya, cambia
   `programator.toml` al sustituto, conservando los comentarios, y retira el viejo.

Las cifras que dependen de encargos de código (la familia de oficio, la de uso y la clase de la
función acotada) solo se comparan entre modelos cuyo código se puntuó igual: todos ejecutando las
pruebas ocultas, o todos leyéndolo con la rúbrica. Si uno se puntuó de una forma y otro de la otra,
o de alguno no se sabe (tandas de antes del 13/11/2026, o mezcladas), esas cifras **se dejan fuera**
de la comparación y de la regla, y `propuesta.md` dice cuáles y por qué en «Cifras que no se
comparan». Si sin ellas no queda nada que comparar, no se retira ni se sustituye nada, y la
propuesta lo dice. Cuéntalo en el informe: para decidir esas cifras hay que volver a pasar el banco
de forma que el código se puntúe igual.

Publica qué se queda, qué se retiró y si cambió el modelo por defecto. La propuesta con todas las
cifras queda en `revisiones/AAAA-MM/propuesta.md`.

## 6. El informe para el Director

Escríbelo en `revisiones/AAAA-MM/informe.md` y resúmelo en tu buzón. Lleva:

- qué se buscó y cuántos candidatos salieron;
- cuáles probaste y por qué esos;
- la tabla de quién gana cada característica, de `propuesta.md`;
- qué se quedó, qué se retiró y si cambió el modelo por defecto;
- lo que conviene saber de cada modelo nuevo y que la tabla no dice (por ejemplo, que se inventa
  APIs que no existen), sacado de los motivos de tus notas;
- lo que no salió bien: candidatos que no se descargaron o no cargaron, tandas sin comprobar.

Es lo único que lee el Director. Que diga claro si hay un modelo nuevo disponible.

## Si algo se corta

Cada paso deja su rastro en el canal y en `revisiones/AAAA-MM/probados.toml`, así que una revisión
cortada a medias se retoma leyendo los dos: lo descargado sigue, una descarga interrumpida se
reanuda, y una tanda a medias se repite entera. Volver a pedir `revisión: probar` con un candidato
ya probado lo prueba otra vez.

## Las mismas piezas, a mano

Programator conserva las órdenes de terminal que hacen cada paso, para usarlas sueltas:
`--revision-buscar`, `--descargar-modelo <repositorio> <fichero>`, `--banco banco --modelo
"<fichero>"`, `--banco-comprobar`, `--banco-informe`,
`--registro-anotar`, `--revision-proponer` y `--retirar-modelo <fichero>`. Las explica
`programator --ayuda`.
