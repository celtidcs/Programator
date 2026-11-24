# Trae el vigilante del canal de Gestor Agentes de su release publicado y lo deja en
# terceros/vigilante/, de donde lo toma empaquetado/contenido.txt.
#
# El paquete de
# Programator lleva gestor-vigilante, con su licencia y su LEEME, sacados del paquete de Gestor
# Agentes de la version que fija empaquetado/version-del-vigilante.txt. Su codigo fuente esta en
# https://github.com/celtidcs/gestor-agentes (GPL-3.0-or-later).
#
# Corre en Windows PowerShell 5.1 y en PowerShell 7, en Windows y en Linux. Necesita gh con acceso
# de lectura al repositorio. Se escribe sin tildes porque PowerShell 5.1 lee como ANSI un guion sin
# BOM.
#
# Uso:
#   ./empaquetado/traer-vigilante.ps1 -Sistema windows
#   ./empaquetado/traer-vigilante.ps1 -Sistema linux
#
# Falla (codigo distinto de cero) si esa version no esta publicada o si su paquete no trae el
# vigilante, la licencia o el LEEME.

param(
    [Parameter(Mandatory = $true)][ValidateSet('windows', 'linux')][string]$Sistema
)

$ErrorActionPreference = 'Stop'
$Raiz = Split-Path -Parent $PSScriptRoot
$Repositorio = 'celtidcs/gestor-agentes'
$Version = (Get-Content -Raw (Join-Path $PSScriptRoot 'version-del-vigilante.txt')).Trim()
$Carpeta = "gestor-agentes-$Version"
$Extension = if ($Sistema -eq 'windows') { 'zip' } else { 'tar.gz' }
$Paquete = "$Carpeta-$Sistema-x86_64.$Extension"
$Vigilante = if ($Sistema -eq 'windows') { 'gestor-vigilante.exe' } else { 'gestor-vigilante' }
# Lo que se saca del paquete de Gestor Agentes: ruta dentro del paquete y nombre en terceros/.
$Piezas = @(
    @{ Dentro = $Vigilante; Fuera = $Vigilante },
    @{ Dentro = 'documentacion/Licencia.txt'; Fuera = 'Licencia.txt' },
    @{ Dentro = 'LEEME.txt'; Fuera = 'LEEME.txt' }
)

$Destino = Join-Path $Raiz 'terceros/vigilante'
$Trabajo = Join-Path $Raiz 'target/traer-vigilante'
if (Test-Path $Trabajo) { Remove-Item -Recurse -Force $Trabajo }
New-Item -ItemType Directory -Force $Trabajo | Out-Null
New-Item -ItemType Directory -Force $Destino | Out-Null

gh release download "v$Version" --repo $Repositorio --pattern $Paquete --dir $Trabajo --clobber
if ($LASTEXITCODE -ne 0) {
    Write-Host "No se pudo bajar $Paquete de la version v$Version de $Repositorio. Esa version tiene que estar publicada."
    exit 1
}

$Archivo = Join-Path $Trabajo $Paquete
$Abierto = Join-Path $Trabajo 'abierto'
New-Item -ItemType Directory -Force $Abierto | Out-Null
if ($Sistema -eq 'windows') {
    Expand-Archive -Path $Archivo -DestinationPath $Abierto -Force
} else {
    tar -xzf $Archivo -C $Abierto
    if ($LASTEXITCODE -ne 0) { Write-Host "No se pudo abrir $Paquete."; exit 1 }
}

foreach ($pieza in $Piezas) {
    $origen = Join-Path (Join-Path $Abierto $Carpeta) $pieza.Dentro
    if (-not (Test-Path -PathType Leaf $origen)) {
        Write-Host "El paquete $Paquete no trae $($pieza.Dentro)."
        exit 1
    }
    Copy-Item -Force $origen (Join-Path $Destino $pieza.Fuera)
}
if ($Sistema -eq 'linux') { chmod +x (Join-Path $Destino $Vigilante) }

Remove-Item -Recurse -Force $Trabajo
Write-Host "Vigilante de Gestor Agentes $Version ($Sistema) en terceros/vigilante/."
exit 0
