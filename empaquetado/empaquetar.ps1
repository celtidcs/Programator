# Empaqueta el portable de Programator segun empaquetado/contenido.txt y comprueba el paquete.
#
# Es el unico empaquetador: lo llaman la publicacion de GitHub (los dos sistemas) y quien quiera
# probar el paquete en local. Corre en Windows PowerShell 5.1 y en PowerShell 7, en Windows y en
# Linux. Se escribe sin tildes porque PowerShell 5.1 lee como ANSI un guion sin BOM.
#
# Uso:
#   ./empaquetado/empaquetar.ps1 -Binario target/release/programator.exe -Sistema windows -Version 1.0.0
#   ./empaquetado/empaquetar.ps1 -Binario target/release/programator -Sistema linux -Version 1.0.0
#
# Deja en -Salida (por defecto target/paquete, que git ignora):
#   windows: programator-<version>-windows-x86_64.zip, plano.
#   linux:   programator-<version>-linux-x86_64.tar.gz, con una carpeta envolvente del mismo
#            nombre, y con permiso de ejecucion en el binario, en preparar-portable.sh y en el
#            vigilante.
#
# Antes hay que traer el vigilante de Gestor Agentes: ./empaquetado/traer-vigilante.ps1.
#
# Falla (codigo distinto de cero) si falta un origen de la lista o si el paquete no lleva
# exactamente lo que dice la lista.

param(
    [Parameter(Mandatory = $true)][string]$Binario,
    [Parameter(Mandatory = $true)][ValidateSet('windows', 'linux')][string]$Sistema,
    [Parameter(Mandatory = $true)][string]$Version,
    [string]$Salida = 'target/paquete'
)

$ErrorActionPreference = 'Stop'
$Raiz = Split-Path -Parent $PSScriptRoot
$Lista = Join-Path $PSScriptRoot 'contenido.txt'
$Nombre = "programator-$Version-$Sistema-x86_64"
$NombreDelBinario = if ($Sistema -eq 'windows') { 'programator.exe' } else { 'programator' }
# Lo que tiene que poder ejecutarse en Linux despues de descomprimir.
$Ejecutables = @($NombreDelBinario, 'preparar-portable.sh', 'vigilante/gestor-vigilante')

function Fallar($mensaje) {
    Write-Host "ERROR: $mensaje" -ForegroundColor Red
    exit 1
}

# Las entradas de la lista que van en este sistema, como objetos { Origen; Destino }.
function Leer-Lista {
    $entradas = @()
    $numero = 0
    foreach ($linea in Get-Content -LiteralPath $Lista -Encoding UTF8) {
        $numero++
        $texto = $linea.Trim()
        if ($texto -eq '' -or $texto.StartsWith('#')) { continue }
        $marca = $null
        if ($texto -match '^\[(windows|linux)\]\s+(.*)$') {
            $marca = $Matches[1]
            $texto = $Matches[2]
        }
        $partes = $texto -split '\s+->\s+'
        if ($partes.Count -ne 2) { Fallar "contenido.txt, linea ${numero}: se esperaba 'origen -> destino'" }
        if ($marca -and $marca -ne $Sistema) { continue }
        $entradas += [pscustomobject]@{ Origen = $partes[0].Trim(); Destino = $partes[1].Trim() }
    }
    return $entradas
}

# Los ficheros que deja una entrada en el paquete, con rutas relativas y barras '/'.
function Ficheros-De($entrada) {
    $origen = Join-Path $Raiz $entrada.Origen
    if ($entrada.Origen.EndsWith('/')) {
        if (-not (Test-Path -LiteralPath $origen -PathType Container)) { Fallar "falta la carpeta $($entrada.Origen)" }
        $base = (Resolve-Path -LiteralPath $origen).Path.TrimEnd('\', '/')
        return Get-ChildItem -LiteralPath $origen -Recurse -File | ForEach-Object {
            $relativa = $_.FullName.Substring($base.Length + 1) -replace '\\', '/'
            [pscustomobject]@{ Desde = $_.FullName; Hacia = $entrada.Destino.TrimEnd('/') + '/' + $relativa }
        }
    }
    if (-not (Test-Path -LiteralPath $origen -PathType Leaf)) { Fallar "falta el fichero $($entrada.Origen)" }
    return [pscustomobject]@{ Desde = $origen; Hacia = $entrada.Destino }
}

# Lo que el paquete lleva, sin carpetas, con barras '/' y sin la carpeta envolvente del tar.gz.
function Contenido-Del-Paquete($paquete) {
    if ($Sistema -eq 'windows') {
        Add-Type -AssemblyName System.IO.Compression.FileSystem
        $zip = [System.IO.Compression.ZipFile]::OpenRead($paquete)
        try {
            return $zip.Entries | Where-Object { $_.Name -ne '' } | ForEach-Object { $_.FullName -replace '\\', '/' }
        } finally { $zip.Dispose() }
    }
    return tar -tzf $paquete | Where-Object { -not $_.EndsWith('/') } | ForEach-Object {
        $_ -replace "^$([regex]::Escape($Nombre))/", ''
    }
}

# --- Preparar ---------------------------------------------------------------
if (-not (Test-Path -LiteralPath $Binario -PathType Leaf)) { Fallar "no existe el binario $Binario" }
New-Item -ItemType Directory -Path $Salida -Force | Out-Null
$Salida = (Resolve-Path -LiteralPath $Salida).Path
$Montaje = Join-Path $Salida $Nombre
if (Test-Path -LiteralPath $Montaje) { Remove-Item -LiteralPath $Montaje -Recurse -Force }
New-Item -ItemType Directory -Path $Montaje | Out-Null

$ficheros = @([pscustomobject]@{ Desde = (Resolve-Path -LiteralPath $Binario).Path; Hacia = $NombreDelBinario })
foreach ($entrada in Leer-Lista) { $ficheros += @(Ficheros-De $entrada) }

$repetidos = $ficheros | Group-Object Hacia | Where-Object { $_.Count -gt 1 }
if ($repetidos) { Fallar "dos entradas dejan el mismo fichero: $(($repetidos | ForEach-Object Name) -join ', ')" }

foreach ($f in $ficheros) {
    $destino = Join-Path $Montaje $f.Hacia
    New-Item -ItemType Directory -Path (Split-Path -Parent $destino) -Force | Out-Null
    Copy-Item -LiteralPath $f.Desde -Destination $destino
}

# --- Comprimir --------------------------------------------------------------
if ($Sistema -eq 'windows') {
    $paquete = Join-Path $Salida "$Nombre.zip"
    if (Test-Path -LiteralPath $paquete) { Remove-Item -LiteralPath $paquete -Force }
    Compress-Archive -Path (Join-Path $Montaje '*') -DestinationPath $paquete -CompressionLevel Optimal
} else {
    foreach ($ejecutable in $Ejecutables) { chmod +x (Join-Path $Montaje $ejecutable) }
    $paquete = Join-Path $Salida "$Nombre.tar.gz"
    tar -czf $paquete -C $Salida $Nombre
    if ($LASTEXITCODE -ne 0) { Fallar "tar no pudo crear $paquete" }
}
Remove-Item -LiteralPath $Montaje -Recurse -Force

# --- Comprobar --------------------------------------------------------------
$esperado = @($ficheros | ForEach-Object Hacia | Sort-Object)
$obtenido = @(Contenido-Del-Paquete $paquete | Sort-Object)
$diferencias = Compare-Object $esperado $obtenido
if ($diferencias) {
    $diferencias | ForEach-Object {
        $lado = if ($_.SideIndicator -eq '<=') { 'falta en el paquete' } else { 'sobra en el paquete' }
        Write-Host "  $lado : $($_.InputObject)"
    }
    Fallar "el paquete no coincide con empaquetado/contenido.txt"
}
if ($Sistema -eq 'linux') {
    $listado = tar -tvzf $paquete
    foreach ($ejecutable in $Ejecutables) {
        $fila = $listado | Where-Object { $_ -match "\s$([regex]::Escape("$Nombre/$ejecutable"))$" }
        if (-not ($fila -match '^-..x')) { Fallar "$ejecutable no tiene permiso de ejecucion en el paquete" }
    }
}

Write-Host "Paquete: $paquete ($($esperado.Count) ficheros, coincide con contenido.txt)" -ForegroundColor Green
# Explicito: quien llama mira $LASTEXITCODE, y sin esto quedaria el de la ultima orden externa.
exit 0
