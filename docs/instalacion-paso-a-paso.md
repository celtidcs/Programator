# Instalación paso a paso, y qué hacer cuando algo no se puede descargar

Programator es un programa pequeño. Lo que ocupa son las dos piezas que necesita para funcionar y
que no vienen con él: el motor de inferencia, que es el programa que de verdad ejecuta el modelo
en la tarjeta gráfica, y el modelo en sí, que son unos trece gigas de fichero. Ninguna de las dos
es nuestra, las dos viven en otros sitios de internet, y por eso hay un script que las trae.

Este documento cuenta qué hace ese script, paso por paso, y sobre todo qué hacer si alguno de esos
pasos falla. Porque va a fallar alguna vez: las direcciones de internet cambian, los proyectos
mueven sus ficheros y las conexiones se cortan. Lo importante es que cuando eso pase no te quedes
con una carpeta inservible y sin saber por dónde seguir.

## Lo primero, para quitarle hierro al asunto

**Programator no está atado ni a ese motor ni a ese modelo.** Las dos piezas se eligen en el
fichero de configuración, en las claves `binario` y `modelo` de la sección `[motor]`. Cualquier
compilación de `llama.cpp` que traiga `llama-server` y cualquier fichero en formato GGUF sirven.

Eso significa que si una descarga falla, no te has quedado sin nada: te falta una pieza que puedes
conseguir de otra forma y poner en su sitio. El peor caso no es «no funciona», es «hay que
terminarlo a mano», y a mano son tres ficheros en dos carpetas.

## Lo que hace falta antes de empezar

Una tarjeta gráfica NVIDIA con sus controladores instalados. Programator funciona sin ella, pero
el modelo se ejecutaría en el procesador y tardaría tanto que no compensa. Con tarjetas de otras
marcas hace falta una compilación distinta del motor, y eso se explica más abajo.

Unos veinte gigas libres en el disco: el modelo ocupa trece, el motor algo más de uno, y los
paquetes comprimidos ocupan lo suyo mientras se descomprimen.

Una conexión que aguante una descarga larga. Trece gigas tardan lo que tarden, y si se corta, el
script sabe continuar donde se quedó.

## Cómo se lanza

En Windows, desde la carpeta de Programator:

```
powershell -ExecutionPolicy Bypass -File preparar-portable.ps1
```

En Linux:

```
chmod +x preparar-portable.sh
./preparar-portable.sh
```

Los dos admiten dos opciones. Con `-SinModelo` en Windows, o `--sin-modelo` en Linux, se salta la
descarga del modelo: sirve cuando ya tienes un GGUF en el disco y prefieres apuntar a él. Con
`-Rehacer` o `--rehacer` se vuelve a descargar todo aunque ya esté, que es lo que se hace cuando
se sospecha que un fichero se corrompió.

**Se puede lanzar tantas veces como haga falta.** Antes de descargar nada comprueba lo que ya está
y se lo salta si es correcto. Si se cortó a mitad, la segunda vez continúa.

## Los tres pasos, uno por uno

### Paso 1: el motor de inferencia

El motor es `llama-server`, del proyecto `llama.cpp`. Programator no lo lleva dentro: lo arranca
como un programa aparte, le habla por HTTP en el puerto que diga la configuración, y lo mata al
cerrarse.

Se descargan dos paquetes, y conviene entender por qué son dos. El primero trae el motor en sí,
que son unos 250 MB en Windows. El segundo trae las bibliotecas de cálculo de NVIDIA, que son casi
400 MB más y que el motor necesita para hablar con la tarjeta. Van separados porque así los
publica el proyecto `llama.cpp`, no porque nosotros los hayamos separado.

La versión que se descarga es la **b10993**, y no es la más reciente a propósito: es la que está
verificada contra Programator. Hay código del arnés que depende de detalles de esa compilación
—por ejemplo, la forma exacta de pasarle la opción `--flash-attn`, que cambió en esa versión—, así
que coger la última sin comprobar puede romper el arranque con un error que no dice nada útil.
Hacer configurable ese número es precisamente lo que traerá la versión 0.11.0.

Hay una diferencia entre sistemas que no es un error del script: en Windows la compilación
publicada va contra CUDA 12.4 y en Linux contra la 12.8. Es lo que publica `llama.cpp` para cada
uno. Tu controlador de NVIDIA tiene que ser igual o más nuevo que esa versión.

Los dos paquetes se descomprimen dentro de `herramientas/`. Al terminar, ahí tiene que haber un
fichero llamado `llama-server.exe` en Windows, o `llama-server` en Linux, junto a un montón de
bibliotecas. Si el ejecutable ha quedado dentro de una subcarpeta, el script de Linux lo sube
solo; el de Windows avisa para que lo muevas tú, porque en Windows ese caso no se ha dado nunca y
prefiere no adivinar.

### Paso 2: el modelo

El modelo que se descarga es **Devstral Small 2 24B**, cuantizado a Q4_K_M, del repositorio
`unsloth/Devstral-Small-2-24B-Instruct-2512-GGUF` en Hugging Face. Son 14.334.446.752 bytes
exactos, trece gigas y pico.

Es el modelo con el que se ha medido todo lo que cuenta la carpeta `evaluacion`: las notas por
tipo de tarea, las cifras de velocidad y el informe de uso real. Si pones otro, esas cifras dejan
de aplicarte y tendrás que medir las tuyas, que para eso está el banco de pruebas.

El fichero se guarda con un nombre más corto, `devstral-small-2-24b-Q4_K_M.gguf`, porque es el que
trae escrito el fichero de configuración de ejemplo.

### Paso 3: la configuración

Si no existe un `programator.toml`, se crea copiando `programator.ejemplo.toml`. Si ya existe, no
se toca: ese fichero es tuyo y probablemente lo has ajustado.

El de ejemplo viene comentado clave a clave, y trae dos cosas que conviene mirar antes de arrancar.
La primera es la sección `[carpeta]`, que trae una ruta de proyecto ficticia: o la cambias por la
tuya o la comentas para que el programa te pregunte al arrancar. La segunda, si estás en Linux, es
la clave `binario` de la sección `[motor]`, que dice `herramientas/llama-server.exe` y tienes que
dejar en `herramientas/llama-server`, sin la extensión.

### Y al final, la comprobación

El script termina lanzando `programator --diagnostico`, que no arranca el ciclo de trabajo: solo
mira la máquina y cuenta lo que ve. Si en la línea que dice «Encaje» aparece cuántas capas del
modelo caben en tu tarjeta, está todo en su sitio.

## Cómo se verifica que lo descargado es lo que debía ser

Cada pieza se comprueba dos veces. Primero por tamaño, que es instantáneo y caza los cortes de
conexión. Después por su suma SHA-256, que es una huella del contenido: si cambia un solo byte, la
huella cambia entera.

Esas sumas están escritas en el propio script, arriba del todo, con nombre y comentario. Puedes
comprobarlas tú mismo en cualquier momento:

```
# Windows
Get-FileHash -Algorithm SHA256 modelos\devstral-small-2-24b-Q4_K_M.gguf

# Linux
sha256sum modelos/devstral-small-2-24b-Q4_K_M.gguf
```

Si la suma no cuadra, el script descarta el fichero y lo vuelve a descargar en vez de seguir
adelante. Un modelo corrompido no falla al descargarse: falla mucho después, con un error del
motor que no apunta a la causa.

## Qué hacer cuando algo no se puede descargar

Aquí está lo que importa de este documento.

Cuando el script no puede traer una pieza, no se limita a fallar. Escribe qué fichero necesitaba,
de qué dirección lo intentaba sacar, cuántos bytes tiene que pesar, cuál es su suma y en qué
carpeta hay que dejarlo. Con eso puedes hacerlo tú a mano y volver a lanzar el script, que se
saltará lo que ya esté bien.

### Si falla la descarga pero la dirección sigue existiendo

Suele ser la conexión. Vuelve a lanzar el script: continúa donde se quedó, no empieza de cero.

Si se repite, descarga el fichero con el navegador desde la dirección que te ha dado el script,
déjalo en la carpeta que te ha indicado y vuelve a lanzarlo. Al encontrarlo, lo verificará y
seguirá.

### Si la dirección del motor ya no existe

El proyecto `llama.cpp` publica una versión por cada compilación, así que son miles. No las borra
—hemos comprobado que conserva las de hace más de un año—, pero podría cambiar de criterio.

Si eso pasa, ve a `https://github.com/ggml-org/llama.cpp/releases` y busca una compilación para tu
sistema con CUDA. Los nombres de los ficheros siguen siempre el mismo patrón: uno que empieza por
`llama-` y otro que empieza por `cudart-`, los dos con `win-cuda` o `ubuntu-cuda` en el nombre y
terminados en `x64`. Necesitas los dos. Descomprímelos dentro de `herramientas/` y comprueba que
`llama-server` queda en la raíz de esa carpeta.

**Ojo con una cosa**, y es el motivo de que la versión esté fijada: una compilación mucho más
nueva puede haber cambiado el nombre o la forma de alguna opción de la línea de órdenes, y
entonces el motor arranca y muere sin decir por qué. Si te pasa, mira el registro que Programator
escribe en la terminal: ahí sale la orden completa con la que lo ha lanzado, y comparándola con la
ayuda de tu versión del motor (`llama-server --help`) se ve enseguida cuál es la opción que
estorba.

### Si la dirección del modelo ya no existe

Esta es la más fácil de resolver, porque el modelo es intercambiable.

Busca en Hugging Face cualquier GGUF que te quepa en la tarjeta. Lo que hace falta es que sea
formato GGUF, que esté cuantizado a algo que quepa —con 16 GiB de tarjeta, un modelo de 24.000
millones de parámetros en Q4_K_M entra justo— y que sea un modelo entrenado para instrucciones,
no uno base.

Déjalo en `modelos/` con el nombre que quieras y apunta a él desde `programator.toml`:

```toml
[motor]
modelo = "modelos/el-que-hayas-elegido.gguf"
```

Y ya está. Lo único que pierdes es que las cifras medidas de la carpeta `evaluacion` dejan de
aplicarte, porque están medidas con otro modelo.

### Si tu tarjeta no es NVIDIA

El script descarga compilaciones con CUDA, que es de NVIDIA. Si tienes una AMD o una Intel, el
proyecto `llama.cpp` publica otras variantes en la misma página de versiones: las que llevan
`rocm` en el nombre para AMD, `sycl` para Intel, y `vulkan` como opción genérica que funciona en
casi todo aunque rinda menos.

Descarga la que te corresponda, descomprímela en `herramientas/` y lanza el script con
`-SinModelo` o `--sin-modelo` para que se ocupe solo del resto.

Hay una limitación que conviene saber: Programator solo sabe medir tarjetas NVIDIA. En Windows lo
hace a través del sistema gráfico y funciona con cualquiera, pero en Linux pregunta a
`nvidia-smi`, que solo existe con controladores de NVIDIA. Si no puede medir, no podrá decidir
cuántas capas caben, así que tendrás que fijarlo tú cambiando `capas_gpu` de `"auto"` a un número
en `programator.toml`. Empieza bajo y ve subiendo hasta que el motor se queje.

### Si no tienes `curl` ni `wget`

En Windows, `curl.exe` viene con el sistema desde la versión 1803 de Windows 10. Si no está,
descarga los ficheros con el navegador y déjalos donde el script te indique.

En Linux, instala uno de los dos con el gestor de paquetes de tu distribución. Casi todas traen
alguno de fábrica.

## Si prefieres hacerlo todo a mano

No hace falta el script para nada. Esto es lo que tiene que haber cuando termina, y puedes
construirlo tú:

```
programator.exe            (o «programator» en Linux)
programator.toml
plantillas/
herramientas/
    llama-server.exe       (o «llama-server»)
    ggml-cuda.dll          y el resto de bibliotecas del paquete
modelos/
    devstral-small-2-24b-Q4_K_M.gguf
```

Las dos carpetas, `herramientas/` y `modelos/`, se llaman así porque es lo que dice el fichero de
configuración de ejemplo. Si prefieres otros nombres u otras rutas, cambia las claves `binario` y
`modelo` de la sección `[motor]` y colócalas donde quieras: se admiten rutas absolutas.

## Cuando termine

La carpeta `descargas/` guarda los paquetes comprimidos del motor. Se puede borrar sin más: solo
sirve para no tener que volver a bajarlos si algún día rehaces la instalación.

A partir de ahí, la puesta en marcha está contada en `plantillas/LEEME-portable.md`, las
limitaciones que conviene conocer antes de usarlo en serio están en `docs/defectos-conocidos.md`,
y las comprobaciones que hay que hacer a mano porque ninguna prueba automática enciende la tarjeta
están en `docs/pruebas-manuales.md`.
