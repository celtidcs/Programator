"""Pruebas que el modelo no ve, sobre su cuota.py.

Uso: python3 comprobar.py <nombre>. Termina con 0 si pasa, 1 si falla y 2 si el nombre no existe.
Los valores esperados se calcularon aparte con la fórmula del enunciado.
"""

import sys

from cuota import cuota_mensual


def valor(argumentos, esperado):
    def prueba():
        obtenido = cuota_mensual(*argumentos)
        print(f"cuota_mensual{argumentos} = {obtenido!r}, se esperaba {esperado}")
        return abs(obtenido - esperado) < 0.005

    return prueba


def rechaza(argumentos):
    def prueba():
        try:
            cuota_mensual(*argumentos)
        except ValueError:
            return True
        return False

    return prueba


def todas(*pruebas):
    return lambda: all(prueba() for prueba in pruebas)


PRUEBAS = {
    "el_valor_del_enunciado": valor((10000, 5, 12), 856.07),
    "una_hipoteca_a_treinta_anos": valor((200000, 3, 360), 843.21),
    "sin_intereses": valor((1200, 0, 12), 100.00),
    "interes_con_decimales": valor((5000, 7.5, 60), 100.19),
    "un_solo_mes": valor((1, 12, 1), 1.01),
    "rechaza_capital_no_positivo": todas(rechaza((0, 5, 12)), rechaza((-100, 5, 12))),
    "rechaza_meses_no_positivos": todas(rechaza((1000, 5, 0)), rechaza((1000, 5, -3))),
    "rechaza_interes_negativo": rechaza((1000, -1, 12)),
}


def main():
    nombre = sys.argv[1] if len(sys.argv) > 1 else ""
    prueba = PRUEBAS.get(nombre)
    if prueba is None:
        print(f"no existe la prueba «{nombre}»", file=sys.stderr)
        return 2
    try:
        pasa = prueba() is True
    except Exception as fallo:  # una excepción inesperada es un fallo de la entrega
        print(f"lanzó {type(fallo).__name__}: {fallo}", file=sys.stderr)
        pasa = False
    print(f"{'PASA ' if pasa else 'FALLA'}  {nombre}")
    return 0 if pasa else 1


if __name__ == "__main__":
    sys.exit(main())
