"""¿Sirve de algo decirle al modelo que no incruste valores y que parta el código en piezas?

Experimento A/B: el **mismo encargo**, una vez sin instrucciones de sistema y otra con el preámbulo
que se las da. Las comprobaciones son mecánicas, no de gusto: se cuenta si los números con
significado tienen nombre, si la lógica está separada de la entrada/salida y cuánto mide la función
más larga.

El encargo está elegido para que la tentación de incrustar sea fuerte: tres cifras de negocio y un
fichero de por medio.

Uso:  python banco-de-principios.py <puerto> <ruta-del-preambulo>
"""

import ast
import json
import re
import sys
import time
import urllib.request
from pathlib import Path

PUERTO = sys.argv[1] if len(sys.argv) > 1 else "8080"
PREAMBULO = Path(sys.argv[2]) if len(sys.argv) > 2 else None
URL = f"http://127.0.0.1:{PUERTO}/v1/chat/completions"

ENCARGO = (
    "Escribe un modulo de Python que procese pedidos de una tienda.\n\n"
    "Lee un fichero CSV con columnas cliente,importe; a cada pedido le aplica un IVA del 21 por "
    "ciento; si el importe con IVA supera los 100 euros aplica un descuento del 10 por ciento; y "
    "escribe un informe en otro fichero con el total por cliente.\n\n"
    "Devuelve solo el codigo, en un bloque."
)


def pedir(sistema: str | None) -> tuple[str, float]:
    mensajes = []
    if sistema:
        mensajes.append({"role": "system", "content": sistema})
    mensajes.append({"role": "user", "content": ENCARGO})
    cuerpo = {
        "model": "devstral",
        "messages": mensajes,
        "max_tokens": 1200,
        "seed": 42,
        "temperature": 0.2,
    }
    peticion = urllib.request.Request(
        URL,
        data=json.dumps(cuerpo).encode("utf-8"),
        headers={"Content-Type": "application/json"},
    )
    inicio = time.time()
    with urllib.request.urlopen(peticion, timeout=900) as r:
        respuesta = json.load(r)
    return respuesta["choices"][0]["message"]["content"], time.time() - inicio


def extraer_codigo(texto: str) -> str:
    if "```" not in texto:
        return texto
    trozos = texto.split("```")
    bloque = trozos[1]
    return bloque.partition("\n")[2] if bloque.partition("\n")[0].strip() else bloque


def analizar(codigo: str) -> dict:
    """Mide lo que se puede medir sobre el código, sin opinar."""
    resultado = {"compila": False}
    try:
        arbol = ast.parse(codigo)
    except SyntaxError:
        return resultado
    resultado["compila"] = True

    funciones = [n for n in ast.walk(arbol) if isinstance(n, ast.FunctionDef)]
    resultado["funciones"] = len(funciones)
    resultado["lineas_funcion_mas_larga"] = max(
        ((f.end_lineno or f.lineno) - f.lineno + 1 for f in funciones), default=0
    )

    # Constantes de módulo en MAYÚSCULAS con valor numérico: la señal de «esto tiene nombre».
    constantes = []
    for nodo in arbol.body:
        if isinstance(nodo, ast.Assign):
            for destino in nodo.targets:
                if isinstance(destino, ast.Name) and destino.id.isupper():
                    if isinstance(nodo.value, ast.Constant) and isinstance(
                        nodo.value.value, (int, float)
                    ):
                        constantes.append(destino.id)
    resultado["constantes_con_nombre"] = constantes

    # Las tres cifras del enunciado: ¿aparecen sueltas dentro de una función?
    cuerpo_funciones = "\n".join(ast.get_source_segment(codigo, f) or "" for f in funciones)
    incrustadas = []
    for cifra in ("0.21", "1.21", "0.10", "0.9", "100"):
        if re.search(rf"(?<![\w.]){re.escape(cifra)}(?![\w.])", cuerpo_funciones):
            incrustadas.append(cifra)
    resultado["cifras_incrustadas_en_funciones"] = incrustadas

    # ¿Separa la lógica de la entrada/salida? Se mira si hay funciones que no tocan ficheros.
    puras = 0
    for f in funciones:
        fuente = ast.get_source_segment(codigo, f) or ""
        if "open(" not in fuente and "csv." not in fuente and "write" not in fuente:
            puras += 1
    resultado["funciones_sin_tocar_ficheros"] = puras
    return resultado


def puntuar(a: dict) -> tuple[int, list[str]]:
    puntos, notas = 0, []

    def comprobar(condicion: bool, texto: str) -> None:
        nonlocal puntos
        puntos += bool(condicion)
        notas.append(f"  {'ok   ' if condicion else 'FALLA'} {texto}")

    comprobar(a.get("compila"), "compila")
    comprobar(
        not a.get("cifras_incrustadas_en_funciones"),
        f"sin cifras incrustadas dentro de funciones (encontradas: "
        f"{a.get('cifras_incrustadas_en_funciones')})",
    )
    comprobar(
        len(a.get("constantes_con_nombre", [])) >= 2,
        f"las cifras tienen nombre (constantes: {a.get('constantes_con_nombre')})",
    )
    comprobar(
        a.get("funciones", 0) >= 3,
        f"parte el trabajo en piezas (funciones: {a.get('funciones')})",
    )
    comprobar(
        0 < a.get("lineas_funcion_mas_larga", 99) <= 20,
        f"ninguna función pasa de 20 líneas (la mayor: "
        f"{a.get('lineas_funcion_mas_larga')})",
    )
    comprobar(
        a.get("funciones_sin_tocar_ficheros", 0) >= 1,
        f"separa el cálculo de la entrada/salida (funciones puras: "
        f"{a.get('funciones_sin_tocar_ficheros')})",
    )
    return puntos, notas


def main() -> None:
    casos = [("SIN instrucciones", None)]
    if PREAMBULO and PREAMBULO.is_file():
        casos.append(("CON el preámbulo", PREAMBULO.read_text(encoding="utf-8")))

    resumen = []
    for etiqueta, sistema in casos:
        print(f"\n===== {etiqueta} =====", flush=True)
        texto, tardado = pedir(sistema)
        codigo = extraer_codigo(texto)
        Path(f"salida-{'sin' if sistema is None else 'con'}-preambulo.py").write_text(
            codigo, encoding="utf-8"
        )
        analisis = analizar(codigo)
        puntos, notas = puntuar(analisis)
        print("\n".join(notas), flush=True)
        print(f"  --> {puntos}/6 en {tardado:.0f}s", flush=True)
        resumen.append((etiqueta, puntos))

    print("\n===== RESUMEN =====")
    for etiqueta, puntos in resumen:
        print(f"  {etiqueta:<22} {puntos}/6")


if __name__ == "__main__":
    main()
