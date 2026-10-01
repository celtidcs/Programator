# Cómo encargarle trabajo a Programator

> **Para Claude, Codex, Gemini y quien venga después.** Lo mantiene el arnés, no el modelo: lo que
> pone aquí no depende de que Programator se acuerde de decirlo.
>
> Léelo antes de mandarle el primer encargo. Está escrito para que no pierdas tiempo pidiéndole
> cosas que no puede hacer, y para que lo que sí puede hacer salga bien a la primera.

## Qué es

Un modelo local —**Devstral Small 2 24B**, cuantizado a Q4_K_M— corriendo en la tarjeta del
Director bajo un arnés que lo confina. No es un agente autónomo y no conviene tratarlo como tal.

Va a unos **18 tokens por segundo**. Una respuesta de código tarda entre uno y tres minutos.

{desempeno}

## Lo que sí puede hacer por su cuenta

{repertorio}

**`leer_fichero` ahorra ventana, pero pedirle muchas lecturas en el mismo encargo tiene un coste
medido.** En vez de pegarle un fichero de 19 KB entero dentro del encargo, dale la ruta y pídele que
lea solo el tramo que necesita acotando el rango de líneas. Para una consulta puntual («¿existe
ya esta constante?») es la vía más barata.

Pero si el encargo es una auditoría o necesita varios tramos de un fichero grande, medido en
NatureLand: pegar el fragmento exacto en el propio encargo, **numerado con su línea real**, entrega
con más fiabilidad que pedirle que lea — tanto en si entrega algo (encadenar varias lecturas en el
mismo encargo ha hecho que el modelo confunda el rótulo `[solicito leer_fichero]` que ve en su
propio historial con una respuesta de texto normal y la repita, perdiendo el encargo entero) como en
la precisión de las líneas que cita después (de un fichero leído, prácticamente ninguna línea citada
coincidía con la real; de un fichero pegado y numerado, la inmensa mayoría sí). El arnés ya detecta
y corrige ese rótulo imitado dándole un aviso en vez de cerrar el encargo, así que el riesgo de
perderlo entero es menor que antes.

**La línea que cita en una auditoría tampoco es fiable, y para eso hay una vía aparte.** Si pegas
`plantillas/formato-auditoria.md` junto al encargo, el arnés comprueba por sí mismo —buscando el
fragmento literal que cite dentro del fichero real— si la línea es correcta, y la corrige si no lo
es. No es magia: si el literal citado aparece más de una vez en el fichero, el arnés no adivina y lo
marca «sin verificar» en vez de inventarse cuál de las dos quiso decir. Fuera de ese formato formal,
sigue sin haber ninguna garantía sobre la línea citada: verifícala tú.

## Lo que NO hace, y no insistas

- **No ejecuta nada.** Ni comandos, ni pruebas, ni compila por su cuenta. Si necesitas que algo se
  ejecute, lo ejecutas tú.
- **No edita ficheros del proyecto.** Lee los que quiera, pero para entregar escribe propuestas en
  `.gestor/candidatos/programator/` y ahí se quedan hasta que alguien las mueva.
- **No aprueba, ni firma, ni acepta nada.** No tiene el verbo. No le pidas un visto bueno.
- **No navega, no busca en internet, no consulta documentación.**
- **No lleva memoria entre encargos.** Cada encargo empieza de cero. Lo que esté **en un fichero del
  proyecto** se lo pides por ruta; lo que no esté en ninguno —una decisión, un acuerdo, lo que se
  entregó ayer— se lo dices en el encargo.

## La limitación que más caro cuesta: se inventa APIs

**Medido el 22/09/2026, y conviene saber la forma exacta del problema.** Se le preguntó por tres
funciones que no existen:

| Lo que se le preguntó | Veces que admitió que no existe |
|---|---|
| `tokio::task::spawn_blocking_scoped` | **0 de 3** — se inventó firma, garantías y ejemplo |
| `#[serde(flatten_deep)]` | 1 de 3 |
| `HashMap::get_or_insert_default_v2` | 3 de 3 |

**El patrón: cuanto más plausible suena lo inventado, más se lo traga.** Lo obviamente falso lo
detecta —el `_v2` de la tercera lo delata—; lo que *podría* existir en esa biblioteca, no.

Y el caso peor tiene una explicación que conviene entender: **alucina justo donde la comunidad
tiene un hueco conocido**. Las tareas con ámbito son una carencia real y muy pedida del async de
Rust —existen `std::thread::scope` y crates que lo intentan, pero Tokio no lo ofrece—, así que
`spawn_blocking_scoped` suena a algo que *debería* existir. Comprobado por dos vías
independientes: el compilador contra Tokio 1.40 (`cannot find function`, sugiriendo
`spawn_blocking`) y la documentación oficial, donde `tokio::task` expone siete funciones y ninguna
es «scoped». De hecho `spawn_blocking` exige `'static` precisamente porque corre en otro hilo: lo
que el modelo prometía es lo que esa API **no puede** dar.

O sea, que el riesgo no está en las preguntas raras: está en las razonables.

**Y no puede comprobarlo él.** No navega, no tiene red y no consulta documentación: lo que sabe de
una biblioteca es lo que recuerda. Verificar contra la documentación es cosa tuya.

**Consecuencia práctica:** no le preguntes por APIs, versiones ni parámetros de bibliotecas, y si
lo haces, **verifica todo lo que diga antes de usarlo**. Para eso tienes tú red y documentación, y
él no: puede leer los ficheros del proyecto, pero no puede consultar la documentación de una
biblioteca.

## Cómo escribir un encargo que salga bien

### 1. Dirígelo bien o no lo verá

Se publica en **tu propio buzón** del canal, con un encabezado nuevo:

```markdown
## Para Programator: nombre corto del encargo

Lo que le pides.
```

**Tres situaciones en las que un encargo no se atiende:**

- **Publicar antes de que Programator avise de que está listo.** La primera vez que Programator se ejecuta en un proyecto nuevo, toma una instantánea inicial del canal para fijar el punto de partida y no atiende textos anteriores. En todos los arranques posteriores, Programator ya conserva su registro y atiende cualquier novedad publicada desde la última lectura. Para no tener que recordar esta distinción, basta con mirar la terminal: Programator anuncia de forma explícita cuándo comienza a escuchar y con qué cadencia de sondeo. Además, escribe su estado en `.gestor/<agente>/latido.json`. Espera a ver ese aviso de listo antes de publicar el encargo.
- **Añadir texto a un encargo anterior.** Programator solo procesa las secciones nuevas. Una línea añadida al final de un encargo antiguo o sin un encabezado propio no se interpreta como una petición de trabajo. Si necesitas corregir o ampliar algo, crea una sección nueva con su propio encabezado.
- **Mencionar a Programator sin un encabezado propio.** Nombrar al agente dentro de un párrafo en el buzón no genera ninguna acción. El arnés requiere un encabezado formal para iniciar la atención del encargo.

### 2. Dile los requisitos, uno por línea

Esto no es opcional: es la diferencia entre que cumpla y que no. **Medido:** el mismo encargo, con
los requisitos escritos como lista dentro del propio encargo, pasó de 1 a 5 comprobaciones
mecánicas sobre 6. Y pidiéndole además que declarara cómo cumplió cada punto, 6 de 6.

```markdown
## Para Programator: parseador de rangos

Escribe en Python una función expandir(entrada: str) -> list[int] que convierta
'1-3,7' en [1,2,3,7].

Requisitos, todos obligatorios:
- Debe llamarse exactamente expandir.
- Lanza ValueError si el rango está invertido.
- Tolera espacios alrededor del guion.
- Prohibido usar eval.

Entrégalo con escribir_propuesta, nombre exacto rangos.py.
Antes de entregar, escribe una línea por requisito diciendo cómo lo has cumplido.
```

Esa última línea vale más de lo que parece.

### 3. Un encargo, una cosa

No le pidas un proyecto. Pídele **un fichero**. Si necesitas tres ficheros, son tres encargos: su
ventana de contexto es de {contexto} tokens —la que hay configurada ahora mismo en esta
instalación, no una cifra de ejemplo— y lo que le quepa de tu encargo es lo que tendrá para
trabajar.

### 4. Dile cómo entregar

Si quieres un fichero: **«entrégalo con `escribir_propuesta`, nombre exacto `X`»**. Su herramienta
necesita dos argumentos, `nombre` y `contenido`, y si se le olvida el segundo el arnés aborta el
encargo. Recordárselo en el encargo evita ese viaje en balde.

## Qué te va a devolver

Publica en `.gestor/canal/programator.md`. Debajo de lo que él escriba, **el arnés añade el
resultado de compilar sus propuestas**:

```
**Comprobación de las propuestas** (la hace el arnés, no el modelo):
- `rangos.py`: pasa la comprobación
```

**Esa línea la escribe el arnés, no él.** Si dice que no pasa, no pasa, lo que él haya escrito
arriba da igual. Y si dice «sin verificar», es que no hay comprobador configurado para esa
extensión: que compile sigue sin saberse.

{comprobadores}

**Cómo configurar un comprobador para tu lenguaje.** Va en `[verificacion.comprobadores]` de tu
`programator.toml`: una extensión y la orden que la comprueba, ya documentado con ejemplos dentro
del propio fichero. Si tu lenguaje tiene referencias cruzadas entre ficheros (un proyecto de
Godot/.NET, de Unity, cualquier framework con un fichero de proyecto), un comprobador contra el
fichero suelto no basta — ahí dentro del mismo fichero hay un ejemplo de cómo apuntar a un script
propio que copie la propuesta sobre una copia de tu proyecto real antes de compilar.

## Falsos positivos que ya se han visto repetirse

En auditorías con vocabulario de principios de diseño (SOLID, inyección de dependencias), confunde
que un método use **campos o métodos de su propia clase** con una violación de Inversión de
Dependencias. Se le ha avisado en varios encargos distintos y ha reaparecido cada vez con una
variación distinta. Si el encargo es de ese tipo, pega junto a él
`plantillas/auditoria-solid.md`, que trae un ejemplo correcto y uno incorrecto de cada principio
—con este falso positivo marcado primero, porque es el que más veces ha costado un encargo— en vez
de escribir el aviso de nuevo cada vez.

También declara haber cumplido una exclusión del encargo (por ejemplo «no toques los colores») en la
misma respuesta donde la incumple. Pedirle que declare cumplimiento por requisito sigue mejorando la
tasa de aciertos de conjunto, pero **esa declaración no es una señal fiable por sí sola**: verifica
siempre contra el fichero real, no contra lo que él diga que hizo.

## Cómo saber si sigue vivo mientras trabaja

Programator escribe su estado en `.gestor/<agente>/latido.json` en cada sondeo del canal, con tres
valores posibles: `reposo` (sin nada que atender), `atendiendo` (generando una respuesta, con el
encargo en curso) y `error` (algo falló, con el motivo). Vigilar la transición de `atendiendo` a
`reposo` o `error` es la forma más rápida de saber cuándo ha terminado, sin esperar a ver algo nuevo
en el canal.

## Resumen para quien tenga prisa

**Pídele:** una función, un fichero, una auditoría de código buscando **formas** —números sueltos,
métodos largos, una clase que hace de todo—, con los requisitos en una lista y diciéndole cómo
entregar.

**No le pidas:** que ejecute algo, que decida una arquitectura, que diagnostique la causa de un
fallo, datos sobre APIs o versiones, un proyecto entero, ni que apruebe nada.

**Y verifica siempre lo que afirme sobre bibliotecas.** Ahí es donde falla, y falla con aplomo.
