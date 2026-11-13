#!/usr/bin/env bash
# Pruebas que el modelo no ve, sobre su ventas.sql.
# Uso: bash probar.sh <nombre>. Termina con 0 si pasa, 1 si falla, 2 si el nombre no existe y 3 si
# falla la preparación de la base, que es un fallo del banco y no de la entrega.
#
# Las columnas de texto se comparan tal cual y las numéricas con margen de media centésima, para
# que 75, 75.0 y 75.00 cuenten igual.

set -u
nombre="${1:-}"
case "$nombre" in
  ejecuta | resultado_con_datos_basicos) datos=basicos ;;
  solo_cuenta_las_ventas_de_2025) datos=fechas ;;
  un_pais_con_menos_de_tres_clientes) datos=pocos ;;
  el_porcentaje_es_sobre_el_total_del_pais) datos=porcentaje ;;
  *) echo "no existe la prueba «$nombre»" >&2; exit 2 ;;
esac

trabajo=$(mktemp -d)
trap 'rm -rf "$trabajo"' EXIT
base="$trabajo/base.db"

if ! sqlite3 -bail "$base" < esquema.sql || ! sqlite3 -bail "$base" < "datos/$datos.sql"; then
  echo "no se pudo preparar la base de datos" >&2
  exit 3
fi

if ! sqlite3 -bail -csv "$base" < ventas.sql > "$trabajo/salida.csv" 2> "$trabajo/error.txt" \
   || [ -s "$trabajo/error.txt" ]; then
  cat "$trabajo/error.txt" >&2
  echo "FALLA  $nombre: SQLite rechazó la consulta"
  exit 1
fi

if [ "$nombre" = "ejecuta" ]; then
  echo "PASA   $nombre"
  exit 0
fi

if awk -F, -v esperado="esperado/$datos.csv" '
  function limpio(v) { gsub(/"/, "", v); gsub(/\r/, "", v); return v }
  function absoluto(v) { return v < 0 ? -v : v }
  BEGIN {
    n = 0
    while ((getline linea < esperado) > 0) { if (linea != "") esp[++n] = linea }
  }
  NF > 0 {
    m++
    if (m > n) { print "sobra la fila " m ": " $0; malo = 1; next }
    split(esp[m], e, ",")
    if (NF != 6) { print "la fila " m " no tiene 6 columnas: " $0; malo = 1; next }
    for (i = 1; i <= 3; i++) if (limpio($i) != e[i]) { print "fila " m ", columna " i ": " $i " en vez de " e[i]; malo = 1 }
    for (i = 4; i <= 6; i++) if (absoluto(limpio($i) + 0 - e[i]) > 0.006) { print "fila " m ", columna " i ": " $i " en vez de " e[i]; malo = 1 }
  }
  END {
    if (m < n) { print "faltan filas: " m " de " n; malo = 1 }
    exit malo ? 1 : 0
  }
' "$trabajo/salida.csv"; then
  echo "PASA   $nombre"
  exit 0
else
  echo "FALLA  $nombre"
  exit 1
fi
