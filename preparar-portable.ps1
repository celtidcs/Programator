# Prepara la carpeta portable de Programator en Windows: descarga el motor de inferencia y el
# modelo, los coloca donde el programa los espera y comprueba que todo cuadra.
#
# Lo que hace, en una frase: lo que tendrias que hacer tu a mano, pero sin equivocarte de version
# ni dejarte un fichero a medias.
#
# Lo que NO hace: no toca nada fuera de esta carpeta, no instala nada en el sistema y no escribe
# en el registro de Windows. Si borras la carpeta, no queda rastro.
#
# Si algo falla, el script te dice que fichero necesitaba, de donde lo intentaba sacar, cuanto
# tiene que pesar y donde dejarlo. Con eso puedes terminar la instalacion a mano. El documento
# «docs/instalacion-paso-a-paso.md» lo explica entero.

param(
    # Salta la descarga del modelo. Util si ya tienes un GGUF y prefieres apuntar a el desde
    # «programator.toml» en vez de bajar otros trece gigas.
    [switch]$SinModelo,
    # Vuelve a descargar aunque el fichero ya este y su suma cuadre.
    [switch]$Rehacer
)

$ErrorActionPreference = "Stop"

# ---------------------------------------------------------------------------------------------
# LO QUE SE VA A DESCARGAR
#
# Todo lo que puede cambiar de una version a otra esta aqui arriba, con nombre y comentario, y no
# escondido en mitad del script. Si manana quieres otra compilacion del motor o otro modelo, se
# cambia aqui y en ningun otro sitio.
# ---------------------------------------------------------------------------------------------

# La compilacion de llama.cpp con la que se ha desarrollado y medido Programator. No es la mas
# reciente a proposito: es la que esta verificada. La 0.11.0 hara configurable este numero.
$VersionDelMotor = "b10993"

# El motor en si: «llama-server.exe» y sus bibliotecas.
$Motor = @{
    Nombre = "llama-$VersionDelMotor-bin-win-cuda-12.4-x64.zip"
    Url    = "https://github.com/ggml-org/llama.cpp/releases/download/$VersionDelMotor/llama-$VersionDelMotor-bin-win-cuda-12.4-x64.zip"
    Sha256 = "525111b022813aca68e8a16dfdb95b26f113dc232348bcab621211dff73efaf3"
    Bytes  = 254197825
}

# La biblioteca de calculo de NVIDIA que el motor necesita para usar la tarjeta. Va aparte porque
# asi la publica llama.cpp, y porque es mas de la mitad del peso.
$Cudart = @{
    Nombre = "cudart-llama-bin-win-cuda-12.4-x64.zip"
    Url    = "https://github.com/ggml-org/llama.cpp/releases/download/$VersionDelMotor/cudart-llama-bin-win-cuda-12.4-x64.zip"
    Sha256 = "8c79a9b226de4b3cacfd1f83d24f962d0773be79f1e7b75c6af4ded7e32ae1d6"
    Bytes  = 391443627
}

# El modelo con el que se ha medido todo lo que dice la carpeta «evaluacion». Programator no esta
# atado a el: cualquier GGUF vale, y se elige en «[motor] modelo» del TOML.
$Modelo = @{
    Nombre  = "Devstral-Small-2-24B-Instruct-2512-Q4_K_M.gguf"
    Url     = "https://huggingface.co/unsloth/Devstral-Small-2-24B-Instruct-2512-GGUF/resolve/main/Devstral-Small-2-24B-Instruct-2512-Q4_K_M.gguf"
    Sha256  = "d14ba9edee1bb4c4996a726deb81e49ae81800a3216f0774634238c380aee496"
    Bytes   = 14334446752
    # Se guarda con un nombre mas corto, que es el que trae escrito el TOML de ejemplo.
    Destino = "devstral-small-2-24b-Q4_K_M.gguf"
}

# Margen de disco que se exige por encima de lo que ocupan las descargas, para no dejar la maquina
# sin sitio justo al terminar.
$MargenDeDiscoGiB = 3

# ---------------------------------------------------------------------------------------------
# UTILIDADES
# ---------------------------------------------------------------------------------------------

function Escribir-Titulo($texto) {
    Write-Host ""
    Write-Host "=== $texto ===" -ForegroundColor Cyan
}

function Escribir-Bien($texto) { Write-Host "  $texto" -ForegroundColor Green }
function Escribir-Nota($texto) { Write-Host "  $texto" -ForegroundColor Gray }
function Escribir-Aviso($texto) { Write-Host "  $texto" -ForegroundColor Yellow }

# Dice que hacer a mano con un fichero que el script no ha podido traer.
#
# Se llama tanto al fallar una descarga como al no cuadrar una suma. En los dos casos la salida es
# la misma: aqui tienes la direccion, el peso y el sitio; hazlo tu y vuelve a lanzar el script,
# que se saltara lo que ya este bien.
function Explicar-Descarga-Manual($pieza, $carpetaDestino, $causa) {
    Write-Host ""
    Write-Host "  NO SE PUDO CONSEGUIR: $($pieza.Nombre)" -ForegroundColor Red
    Write-Host "  Causa: $causa" -ForegroundColor Red
    Write-Host ""
    Write-Host "  Que hacer a mano:" -ForegroundColor Yellow
    Write-Host "    1. Abre esta direccion en el navegador:"
    Write-Host "       $($pieza.Url)"
    Write-Host "    2. Comprueba que el fichero pesa exactamente $($pieza.Bytes) bytes."
    Write-Host "    3. Dejalo en:  $carpetaDestino"
    Write-Host "    4. Vuelve a lanzar este script: se saltara lo que ya este bien."
    Write-Host ""
    Write-Host "    Su suma SHA-256, por si quieres comprobarla:"
    Write-Host "       $($pieza.Sha256)"
    Write-Host "    En PowerShell:  Get-FileHash -Algorithm SHA256 <fichero>"
    Write-Host ""
    Write-Host "  Si la direccion ya no existe, el documento de instalacion explica de donde sacar" -ForegroundColor Yellow
    Write-Host "  un sustituto y que hace falta que cumpla. Es «INSTALACION.md» si vienes del paquete," -ForegroundColor Yellow
    Write-Host "  o «docs/instalacion-paso-a-paso.md» si has clonado el repositorio." -ForegroundColor Yellow
}

# Traduce el codigo con el que termino «curl.exe» a una frase que se entienda.
#
# Un «codigo 22» no le dice nada a nadie. Saber que el servidor contesto que ese fichero ya no
# existe, en cambio, te manda derecho a la parte del documento que explica de donde sacar un
# sustituto. Los codigos son los que documenta el propio curl.
function Explicar-Codigo-De-Curl($codigo) {
    switch ($codigo) {
        6  { return "no se pudo resolver el nombre del servidor: parece que no hay conexion, o un DNS que no responde" }
        7  { return "no se pudo conectar con el servidor: puede ser un cortafuegos, un proxy o que el servidor este caido" }
        22 { return "el servidor contesto con un error. Lo mas probable es que ese fichero ya no este en esa direccion" }
        23 { return "fallo al escribir en el disco: comprueba el espacio libre y los permisos de esta carpeta" }
        28 { return "se agoto el tiempo de espera: la conexion es demasiado lenta o se ha quedado parada" }
        35 { return "fallo al establecer la conexion segura: revisa la fecha y hora del sistema, y si hay un proxy de por medio" }
        56 { return "la conexion se corto mientras se recibian los datos" }
        60 { return "no se pudo verificar el certificado del servidor: suele pasar detras de un proxy corporativo" }
        default { return "«curl.exe» termino con el codigo $codigo" }
    }
}

# Comprueba que un fichero es el que decimos que es: primero el peso, que es instantaneo, y solo
# despues la suma, que en trece gigas tarda un rato.
function Es-El-Fichero-Correcto($ruta, $pieza) {
    if (-not (Test-Path $ruta)) { return $false }
    $tamano = (Get-Item $ruta).Length
    if ($tamano -ne $pieza.Bytes) {
        Escribir-Nota "«$($pieza.Nombre)» esta pero pesa $tamano bytes y deberia pesar $($pieza.Bytes). Se descarta."
        return $false
    }
    Escribir-Nota "Comprobando la suma de «$($pieza.Nombre)»... (en ficheros grandes tarda un poco)"
    $suma = (Get-FileHash -Algorithm SHA256 -Path $ruta).Hash.ToLower()
    if ($suma -ne $pieza.Sha256.ToLower()) {
        Escribir-Nota "La suma no cuadra. Se descarta y se vuelve a descargar."
        return $false
    }
    return $true
}

# Descarga con «curl.exe», que viene con Windows desde la version 1803 y sabe reanudar.
#
# No se usa «Invoke-WebRequest» a proposito: en Windows PowerShell 5.1 se guarda la respuesta
# entera en memoria antes de escribirla, y con un fichero de trece gigas eso no acaba bien.
function Descargar($pieza, $destino) {
    $curl = Get-Command curl.exe -ErrorAction SilentlyContinue
    if (-not $curl) {
        return "no se encontro «curl.exe», que viene con Windows 10 a partir de la version 1803"
    }
    Write-Host "  Descargando $($pieza.Nombre)..."
    Write-Host "  (son $([math]::Round($pieza.Bytes / 1GB, 2)) GiB; se puede cortar y reanudar volviendo a lanzar el script)"
    # «curl.exe» escribe su barra de progreso por la salida de errores, y con
    # «$ErrorActionPreference = Stop» PowerShell trata eso como un error fatal: la descarga se
    # abortaba nada mas empezar, sin haber fallado nada. Se baja la guardia solo alrededor de esta
    # llamada y se decide por el codigo de salida, que es lo que de verdad dice si fue bien.
    $guardiaPrevia = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    # «-C -» reanuda donde se quedo, «-L» sigue las redirecciones y «--fail» convierte un 404 en
    # un error de verdad en vez de guardar la pagina de error como si fuera el fichero.
    & curl.exe -L --fail -C - --progress-bar -o $destino $pieza.Url
    $codigo = $LASTEXITCODE
    $ErrorActionPreference = $guardiaPrevia

    # El 33 es «el servidor no admite reanudar». Pasa cuando el fichero local ya esta entero o
    # cuando la direccion redirige a un sitio que no lo permite: se reintenta desde cero.
    if ($codigo -eq 33) {
        Remove-Item -Force $destino -ErrorAction SilentlyContinue
        $ErrorActionPreference = "Continue"
        & curl.exe -L --fail --progress-bar -o $destino $pieza.Url
        $codigo = $LASTEXITCODE
        $ErrorActionPreference = $guardiaPrevia
    }

    if ($codigo -ne 0) {
        return (Explicar-Codigo-De-Curl $codigo)
    }
    return $null
}

# Trae una pieza si hace falta: la salta si ya esta bien, la descarga si no, y verifica siempre.
# Devuelve $true si al terminar el fichero esta y es el correcto.
function Asegurar-Pieza($pieza, $carpeta) {
    $destino = Join-Path $carpeta $pieza.Nombre

    if ((-not $Rehacer) -and (Es-El-Fichero-Correcto $destino $pieza)) {
        Escribir-Bien "«$($pieza.Nombre)» ya estaba y es correcto. Se salta."
        return $true
    }

    $fallo = Descargar $pieza $destino
    if ($fallo) {
        Explicar-Descarga-Manual $pieza $carpeta $fallo
        return $false
    }

    if (-not (Es-El-Fichero-Correcto $destino $pieza)) {
        Explicar-Descarga-Manual $pieza $carpeta "el fichero descargado no coincide con lo esperado"
        return $false
    }

    Escribir-Bien "«$($pieza.Nombre)» descargado y verificado."
    return $true
}

# ---------------------------------------------------------------------------------------------
# COMPROBACIONES ANTES DE EMPEZAR
# ---------------------------------------------------------------------------------------------

Escribir-Titulo "Preparando la carpeta portable de Programator"

$Aqui = $PSScriptRoot
Set-Location $Aqui

if (-not (Test-Path (Join-Path $Aqui "programator.ejemplo.toml"))) {
    Write-Host "ALTO: este script tiene que ejecutarse dentro de la carpeta de Programator," -ForegroundColor Red
    Write-Host "      la que trae «programator.ejemplo.toml». Aqui no esta." -ForegroundColor Red
    exit 1
}

# El espacio necesario: las descargas mas lo que ocupan al descomprimirse, mas un margen.
$NecesarioGiB = [math]::Round(($Motor.Bytes + $Cudart.Bytes) * 2 / 1GB, 1)
if (-not $SinModelo) { $NecesarioGiB = $NecesarioGiB + [math]::Round($Modelo.Bytes / 1GB, 1) }
$NecesarioGiB = $NecesarioGiB + $MargenDeDiscoGiB

$unidad = (Get-Item $Aqui).PSDrive.Name
$libreGiB = [math]::Round((Get-PSDrive $unidad).Free / 1GB, 1)
Write-Host "  Espacio libre en $($unidad):  $libreGiB GiB     Hace falta: unos $NecesarioGiB GiB"
if ($libreGiB -lt $NecesarioGiB) {
    Write-Host "ALTO: no hay sitio suficiente. Libera espacio o usa «-SinModelo» si ya tienes uno." -ForegroundColor Red
    exit 1
}

$smi = Get-Command nvidia-smi -ErrorAction SilentlyContinue
if ($smi) {
    Escribir-Bien "Tarjeta NVIDIA detectada. El motor podra usar la GPU."
} else {
    Escribir-Aviso "No se ha encontrado «nvidia-smi», asi que probablemente no hay tarjeta NVIDIA"
    Escribir-Aviso "con sus controladores instalados. La instalacion sigue, pero el modelo correra"
    Escribir-Aviso "por CPU y sera muy lento. Con una tarjeta de otra marca, hara falta una"
    Escribir-Aviso "compilacion distinta de llama.cpp: mira «docs/instalacion-paso-a-paso.md»."
}

# ---------------------------------------------------------------------------------------------
# EL MOTOR
# ---------------------------------------------------------------------------------------------

Escribir-Titulo "1 de 3. El motor de inferencia ($VersionDelMotor)"

$Herramientas = Join-Path $Aqui "herramientas"
$Descargas = Join-Path $Aqui "descargas"
New-Item -ItemType Directory -Force -Path $Herramientas | Out-Null
New-Item -ItemType Directory -Force -Path $Descargas | Out-Null

$servidor = Join-Path $Herramientas "llama-server.exe"
if ((Test-Path $servidor) -and (-not $Rehacer)) {
    Escribir-Bien "«herramientas/llama-server.exe» ya esta. Se salta el motor entero."
} else {
    if (-not (Asegurar-Pieza $Motor $Descargas)) { exit 1 }
    if (-not (Asegurar-Pieza $Cudart $Descargas)) { exit 1 }

    Write-Host "  Descomprimiendo en «herramientas/»..."
    Expand-Archive -Path (Join-Path $Descargas $Motor.Nombre) -DestinationPath $Herramientas -Force
    Expand-Archive -Path (Join-Path $Descargas $Cudart.Nombre) -DestinationPath $Herramientas -Force

    if (-not (Test-Path $servidor)) {
        Write-Host "ALTO: los dos paquetes se han descomprimido pero no aparece «llama-server.exe»." -ForegroundColor Red
        Write-Host "      Mira dentro de «herramientas/»: puede que la compilacion haya cambiado de" -ForegroundColor Red
        Write-Host "      estructura y los ficheros esten en una subcarpeta. Muevelos a la raiz de" -ForegroundColor Red
        Write-Host "      «herramientas/» y vuelve a lanzar el script." -ForegroundColor Red
        exit 1
    }
    Escribir-Bien "Motor colocado en «herramientas/»."
}

# ---------------------------------------------------------------------------------------------
# EL MODELO
# ---------------------------------------------------------------------------------------------

Escribir-Titulo "2 de 3. El modelo"

$Modelos = Join-Path $Aqui "modelos"
New-Item -ItemType Directory -Force -Path $Modelos | Out-Null
$rutaModelo = Join-Path $Modelos $Modelo.Destino

if ($SinModelo) {
    Escribir-Aviso "Saltado por «-SinModelo». Acuerdate de apuntar «[motor] modelo» de tu"
    Escribir-Aviso "«programator.toml» al fichero GGUF que quieras usar."
} elseif ((Test-Path $rutaModelo) -and (-not $Rehacer)) {
    Escribir-Bien "«modelos/$($Modelo.Destino)» ya esta. Se salta."
} else {
    if (-not (Asegurar-Pieza $Modelo $Modelos)) { exit 1 }
    Move-Item -Force -Path (Join-Path $Modelos $Modelo.Nombre) -Destination $rutaModelo
    Escribir-Bien "Modelo colocado como «modelos/$($Modelo.Destino)»."
}

# ---------------------------------------------------------------------------------------------
# LA CONFIGURACION, Y LA COMPROBACION FINAL
# ---------------------------------------------------------------------------------------------

Escribir-Titulo "3 de 3. La configuracion"

$toml = Join-Path $Aqui "programator.toml"
if (Test-Path $toml) {
    Escribir-Bien "«programator.toml» ya existe. No se toca: es tuyo."
} else {
    Copy-Item (Join-Path $Aqui "programator.ejemplo.toml") $toml
    Escribir-Bien "Creado «programator.toml» a partir del ejemplo."
    Escribir-Nota "Repasalo: viene comentado clave a clave y trae una ruta de proyecto de ejemplo"
    Escribir-Nota "en la seccion [carpeta] que seguramente quieras cambiar o comentar."
}

Escribir-Titulo "Comprobacion"

$exe = Join-Path $Aqui "programator.exe"
if (Test-Path $exe) {
    Write-Host ""
    & $exe --diagnostico
    Write-Host ""
    Escribir-Bien "Si el «Encaje» de arriba dice cuantas capas caben en tu tarjeta, esta listo."
    Escribir-Nota "Arranca con «programator.exe» y te pedira la carpeta del proyecto."
} else {
    Escribir-Aviso "No se ha encontrado «programator.exe» en esta carpeta."
    Escribir-Aviso "Si has clonado el repositorio en vez de descargar el paquete portable,"
    Escribir-Aviso "compilalo con «cargo build --release» y copia «target/release/programator.exe» aqui."
}

Write-Host ""
Escribir-Nota "La carpeta «descargas/» guarda los paquetes comprimidos del motor. Puedes borrarla:"
Escribir-Nota "solo sirve para no tener que volver a bajarlos si algun dia rehaces la instalacion."
Write-Host ""
