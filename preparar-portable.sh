#!/usr/bin/env bash
# Prepara la carpeta portable de Programator en Linux: descarga el motor de inferencia y el
# modelo, los coloca donde el programa los espera y comprueba que todo cuadra.
#
# Lo que hace, en una frase: lo que tendrias que hacer tu a mano, pero sin equivocarte de version
# ni dejarte un fichero a medias.
#
# Lo que NO hace: no toca nada fuera de esta carpeta, no instala paquetes y no pide permisos de
# administrador. Si borras la carpeta, no queda rastro.
#
# Si algo falla, el script te dice que fichero necesitaba, de donde lo intentaba sacar, cuanto
# tiene que pesar y donde dejarlo. Con eso puedes terminar la instalacion a mano. El documento
# «docs/instalacion-paso-a-paso.md» lo explica entero.
#
# Uso:  ./preparar-portable.sh [--sin-modelo] [--rehacer]

set -u

SIN_MODELO=0
REHACER=0
# Donde «descargar» deja el motivo del fallo, para que «asegurar_pieza» lo cuente sin tener
# que capturar su salida. Capturarla obligaba a desviar la barra de progreso a «/dev/tty»,
# que no existe cuando el script se ejecuta sin una terminal delante: el propio script se
# convertia entonces en la causa del fallo que decia explicar.
CAUSA=""
for argumento in "$@"; do
    case "$argumento" in
        --sin-modelo) SIN_MODELO=1 ;;
        --rehacer)    REHACER=1 ;;
        *)
            echo "No entiendo «$argumento». Se admiten «--sin-modelo» y «--rehacer»."
            exit 1
            ;;
    esac
done

# ---------------------------------------------------------------------------------------------
# LO QUE SE VA A DESCARGAR
#
# Todo lo que puede cambiar de una version a otra esta aqui arriba, con nombre y comentario, y no
# escondido en mitad del script.
# ---------------------------------------------------------------------------------------------

# La compilacion de llama.cpp con la que se ha desarrollado y medido Programator. No es la mas
# reciente a proposito: es la que esta verificada. La 0.11.0 hara configurable este numero.
VERSION_DEL_MOTOR="b10993"

# Ojo a una diferencia con Windows: alli la compilacion publicada va contra CUDA 12.4 y aqui
# contra la 12.8. No es una incoherencia del script, es lo que publica llama.cpp para cada
# sistema. Tu controlador tiene que ser igual o mas nuevo que esa version.
MOTOR_NOMBRE="llama-${VERSION_DEL_MOTOR}-bin-ubuntu-cuda-12.8-x64.tar.gz"
MOTOR_URL="https://github.com/ggml-org/llama.cpp/releases/download/${VERSION_DEL_MOTOR}/${MOTOR_NOMBRE}"
MOTOR_SHA="4c31cc63e8a9436a113875e538bc59685ad927f44ee8d992b26789730ecb8f98"
MOTOR_BYTES=168895037

# La biblioteca de calculo de NVIDIA que el motor necesita para usar la tarjeta.
CUDART_NOMBRE="cudart-llama-${VERSION_DEL_MOTOR}-bin-ubuntu-cuda-12.8-x64.tar.gz"
CUDART_URL="https://github.com/ggml-org/llama.cpp/releases/download/${VERSION_DEL_MOTOR}/${CUDART_NOMBRE}"
CUDART_SHA="2133934a926943ab2edbf48f655ad89f694416f93b65ffc4aac6158cb003e99e"
CUDART_BYTES=594373452

# El modelo con el que se ha medido todo lo que dice la carpeta «evaluacion». Programator no esta
# atado a el: cualquier GGUF vale, y se elige en «[motor] modelo» del TOML.
MODELO_NOMBRE="Devstral-Small-2-24B-Instruct-2512-Q4_K_M.gguf"
MODELO_URL="https://huggingface.co/unsloth/Devstral-Small-2-24B-Instruct-2512-GGUF/resolve/main/${MODELO_NOMBRE}"
MODELO_SHA="d14ba9edee1bb4c4996a726deb81e49ae81800a3216f0774634238c380aee496"
MODELO_BYTES=14334446752
# Se guarda con un nombre mas corto, que es el que trae escrito el TOML de ejemplo.
MODELO_DESTINO="devstral-small-2-24b-Q4_K_M.gguf"

# Margen de disco que se exige por encima de lo que ocupan las descargas.
MARGEN_DE_DISCO_GIB=3

# ---------------------------------------------------------------------------------------------
# UTILIDADES
# ---------------------------------------------------------------------------------------------

titulo() { printf '\n=== %s ===\n' "$1"; }
bien()   { printf '  %s\n' "$1"; }
nota()   { printf '  %s\n' "$1"; }
aviso()  { printf '  %s\n' "$1"; }
alto()   { printf 'ALTO: %s\n' "$1" >&2; }

# Dice que hacer a mano con un fichero que el script no ha podido traer.
explicar_descarga_manual() {
    local nombre="$1" url="$2" bytes="$3" sha="$4" carpeta="$5" causa="$6"
    cat <<FIN

  NO SE PUDO CONSEGUIR: $nombre
  Causa: $causa

  Que hacer a mano:
    1. Abre esta direccion en el navegador, o usa «wget»:
       $url
    2. Comprueba que el fichero pesa exactamente $bytes bytes.
    3. Dejalo en:  $carpeta
    4. Vuelve a lanzar este script: se saltara lo que ya este bien.

    Su suma SHA-256, por si quieres comprobarla:
       $sha
    En la terminal:  sha256sum <fichero>

  Si la direccion ya no existe, el documento de instalacion explica de donde sacar un
  sustituto y que hace falta que cumpla. Es «INSTALACION.md» si vienes del paquete, o
  «docs/instalacion-paso-a-paso.md» si has clonado el repositorio.

FIN
}

# Traduce el codigo con el que termino «curl» a una frase que se entienda.
#
# Un «codigo 22» no le dice nada a nadie. Saber que el servidor contesto que ese fichero ya no
# existe, en cambio, te manda derecho a la parte del documento que explica de donde sacar un
# sustituto. Los codigos son los que documenta el propio curl.
explicar_codigo_de_curl() {
    case "$1" in
        6)  echo "no se pudo resolver el nombre del servidor: parece que no hay conexion, o un DNS que no responde" ;;
        7)  echo "no se pudo conectar con el servidor: puede ser un cortafuegos, un proxy o que el servidor este caido" ;;
        22) echo "el servidor contesto con un error. Lo mas probable es que ese fichero ya no este en esa direccion" ;;
        23) echo "fallo al escribir en el disco: comprueba el espacio libre y los permisos de esta carpeta" ;;
        28) echo "se agoto el tiempo de espera: la conexion es demasiado lenta o se ha quedado parada" ;;
        35) echo "fallo al establecer la conexion segura: revisa la fecha y hora del sistema, y si hay un proxy de por medio" ;;
        56) echo "la conexion se corto mientras se recibian los datos" ;;
        60) echo "no se pudo verificar el certificado del servidor: suele pasar detras de un proxy corporativo" ;;
        *)  echo "«curl» termino con el codigo $1" ;;
    esac
}

# Comprueba que un fichero es el que decimos que es: primero el peso, que es instantaneo, y solo
# despues la suma, que en trece gigas tarda un rato.
es_el_fichero_correcto() {
    local ruta="$1" bytes="$2" sha="$3" nombre="$4"
    [ -f "$ruta" ] || return 1
    local tamano
    tamano=$(stat -c%s "$ruta")
    if [ "$tamano" != "$bytes" ]; then
        nota "«$nombre» esta pero pesa $tamano bytes y deberia pesar $bytes. Se descarta."
        return 1
    fi
    nota "Comprobando la suma de «$nombre»... (en ficheros grandes tarda un poco)"
    local calculada
    calculada=$(sha256sum "$ruta" | cut -d' ' -f1)
    if [ "$calculada" != "$sha" ]; then
        nota "La suma no cuadra. Se descarta y se vuelve a descargar."
        return 1
    fi
    return 0
}

# Descarga reanudando si se corto. Se prefiere «curl» y se acepta «wget»: en una instalacion
# minima puede faltar uno de los dos, pero rara vez los dos.
descargar() {
    local url="$1" destino="$2" bytes="$3"
    printf '  Descargando %s...\n' "$(basename "$destino")"
    printf '  (son %s GiB; se puede cortar y reanudar volviendo a lanzar el script)\n' \
        "$(awk "BEGIN {printf \"%.2f\", $bytes/1073741824}")"
    if command -v curl >/dev/null 2>&1; then
        # «-C -» reanuda donde se quedo, «-L» sigue redirecciones y «--fail» convierte un 404 en
        # un error de verdad en vez de guardar la pagina de error como si fuera el fichero.
        curl -L --fail -C - --progress-bar -o "$destino" "$url" && return 0
        CAUSA=$(explicar_codigo_de_curl "$?")
        return 1
    fi
    if command -v wget >/dev/null 2>&1; then
        wget --continue --output-document="$destino" "$url" && return 0
        CAUSA="«wget» termino con el codigo $?"
        return 1
    fi
    CAUSA="no se encontro ni «curl» ni «wget»; instala uno de los dos"
    return 1
}

# Trae una pieza si hace falta: la salta si ya esta bien, la descarga si no, y verifica siempre.
asegurar_pieza() {
    local nombre="$1" url="$2" sha="$3" bytes="$4" carpeta="$5"
    local destino="$carpeta/$nombre"

    if [ "$REHACER" -eq 0 ] && es_el_fichero_correcto "$destino" "$bytes" "$sha" "$nombre"; then
        bien "«$nombre» ya estaba y es correcto. Se salta."
        return 0
    fi

    CAUSA=""
    if ! descargar "$url" "$destino" "$bytes"; then
        explicar_descarga_manual "$nombre" "$url" "$bytes" "$sha" "$carpeta" \
            "${CAUSA:-fallo de red}"
        return 1
    fi

    if ! es_el_fichero_correcto "$destino" "$bytes" "$sha" "$nombre"; then
        explicar_descarga_manual "$nombre" "$url" "$bytes" "$sha" "$carpeta" \
            "el fichero descargado no coincide con lo esperado"
        return 1
    fi

    bien "«$nombre» descargado y verificado."
    return 0
}

# ---------------------------------------------------------------------------------------------
# COMPROBACIONES ANTES DE EMPEZAR
# ---------------------------------------------------------------------------------------------

titulo "Preparando la carpeta portable de Programator"

AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$AQUI"

if [ ! -f "$AQUI/programator.ejemplo.toml" ]; then
    alto "este script tiene que ejecutarse dentro de la carpeta de Programator, la que trae"
    alto "«programator.ejemplo.toml». Aqui no esta."
    exit 1
fi

NECESARIO_GIB=$(awk "BEGIN {printf \"%.0f\", ($MOTOR_BYTES + $CUDART_BYTES) * 2 / 1073741824 + $MARGEN_DE_DISCO_GIB}")
if [ "$SIN_MODELO" -eq 0 ]; then
    NECESARIO_GIB=$(awk "BEGIN {printf \"%.0f\", $NECESARIO_GIB + $MODELO_BYTES / 1073741824}")
fi
LIBRE_GIB=$(df -BG --output=avail "$AQUI" | tail -1 | tr -dc '0-9')
printf '  Espacio libre aqui:  %s GiB     Hace falta: unos %s GiB\n' "$LIBRE_GIB" "$NECESARIO_GIB"
if [ "$LIBRE_GIB" -lt "$NECESARIO_GIB" ]; then
    alto "no hay sitio suficiente. Libera espacio o usa «--sin-modelo» si ya tienes uno."
    exit 1
fi

if command -v nvidia-smi >/dev/null 2>&1; then
    bien "Tarjeta NVIDIA detectada. El motor podra usar la GPU."
    nota "$(nvidia-smi --query-gpu=name,memory.total --format=csv,noheader 2>/dev/null || true)"
else
    aviso "No se ha encontrado «nvidia-smi», asi que probablemente no hay tarjeta NVIDIA con sus"
    aviso "controladores instalados. La instalacion sigue, pero el modelo correra por CPU y sera"
    aviso "muy lento. Programator tampoco podra medir la tarjeta, asi que tendras que fijar a mano"
    aviso "«[motor] capas_gpu» en vez de dejarlo en «auto»."
fi

# ---------------------------------------------------------------------------------------------
# EL MOTOR
# ---------------------------------------------------------------------------------------------

titulo "1 de 3. El motor de inferencia ($VERSION_DEL_MOTOR)"

HERRAMIENTAS="$AQUI/herramientas"
DESCARGAS="$AQUI/descargas"
mkdir -p "$HERRAMIENTAS" "$DESCARGAS"

if [ -x "$HERRAMIENTAS/llama-server" ] && [ "$REHACER" -eq 0 ]; then
    bien "«herramientas/llama-server» ya esta. Se salta el motor entero."
else
    asegurar_pieza "$MOTOR_NOMBRE" "$MOTOR_URL" "$MOTOR_SHA" "$MOTOR_BYTES" "$DESCARGAS" || exit 1
    asegurar_pieza "$CUDART_NOMBRE" "$CUDART_URL" "$CUDART_SHA" "$CUDART_BYTES" "$DESCARGAS" || exit 1

    echo "  Descomprimiendo en «herramientas/»..."
    tar -xzf "$DESCARGAS/$MOTOR_NOMBRE" -C "$HERRAMIENTAS"
    tar -xzf "$DESCARGAS/$CUDART_NOMBRE" -C "$HERRAMIENTAS"

    # Los paquetes de Linux suelen traer los binarios dentro de «build/bin/». Si es asi, se suben
    # a la raiz de «herramientas/», que es donde apunta el TOML de ejemplo.
    if [ ! -f "$HERRAMIENTAS/llama-server" ]; then
        encontrado=$(find "$HERRAMIENTAS" -type f -name llama-server -print -quit)
        if [ -n "$encontrado" ]; then
            origen=$(dirname "$encontrado")
            nota "Los binarios venian en «${origen#"$HERRAMIENTAS"/}». Se suben a «herramientas/»."
            find "$origen" -maxdepth 1 -type f -exec mv -f {} "$HERRAMIENTAS/" \;
        fi
    fi

    if [ ! -f "$HERRAMIENTAS/llama-server" ]; then
        alto "los dos paquetes se han descomprimido pero no aparece «llama-server»."
        alto "Mira dentro de «herramientas/», mueve el binario y sus bibliotecas a la raiz de esa"
        alto "carpeta y vuelve a lanzar el script."
        exit 1
    fi
    chmod +x "$HERRAMIENTAS/llama-server"
    bien "Motor colocado en «herramientas/»."
fi

# ---------------------------------------------------------------------------------------------
# EL MODELO
# ---------------------------------------------------------------------------------------------

titulo "2 de 3. El modelo"

MODELOS="$AQUI/modelos"
mkdir -p "$MODELOS"
RUTA_MODELO="$MODELOS/$MODELO_DESTINO"

if [ "$SIN_MODELO" -eq 1 ]; then
    aviso "Saltado por «--sin-modelo». Acuerdate de apuntar «[motor] modelo» de tu"
    aviso "«programator.toml» al fichero GGUF que quieras usar."
elif [ -f "$RUTA_MODELO" ] && [ "$REHACER" -eq 0 ]; then
    bien "«modelos/$MODELO_DESTINO» ya esta. Se salta."
else
    asegurar_pieza "$MODELO_NOMBRE" "$MODELO_URL" "$MODELO_SHA" "$MODELO_BYTES" "$MODELOS" || exit 1
    mv -f "$MODELOS/$MODELO_NOMBRE" "$RUTA_MODELO"
    bien "Modelo colocado como «modelos/$MODELO_DESTINO»."
fi

# ---------------------------------------------------------------------------------------------
# LA CONFIGURACION, Y LA COMPROBACION FINAL
# ---------------------------------------------------------------------------------------------

titulo "3 de 3. La configuracion"

TOML="$AQUI/programator.toml"
if [ -f "$TOML" ]; then
    bien "«programator.toml» ya existe. No se toca: es tuyo."
else
    cp "$AQUI/programator.ejemplo.toml" "$TOML"
    bien "Creado «programator.toml» a partir del ejemplo."
    nota "Repasalo, y cambia «[motor] binario» a «herramientas/llama-server», sin «.exe»:"
    nota "el ejemplo viene escrito para Windows."
fi

titulo "Comprobacion"

if [ -x "$AQUI/programator" ]; then
    echo
    "$AQUI/programator" --diagnostico
    echo
    bien "Si el «Encaje» de arriba dice cuantas capas caben en tu tarjeta, esta listo."
    nota "Arranca con «./programator» y te pedira la carpeta del proyecto."
else
    aviso "No se ha encontrado el ejecutable «programator» en esta carpeta."
    aviso "Compilalo con «cargo build --release» y copia «target/release/programator» aqui."
fi

echo
nota "La carpeta «descargas/» guarda los paquetes comprimidos del motor. Puedes borrarla:"
nota "solo sirve para no tener que volver a bajarlos si algun dia rehaces la instalacion."
echo
