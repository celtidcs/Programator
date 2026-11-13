"""Pruebas que el modelo no ve, sobre su reintentos.py.

Uso: python3 adversarias.py <nombre>. Termina con 0 si pasa, 1 si falla y 2 si el nombre no
existe. La espera se intercepta sin dormir de verdad, tanto si el modelo llama a time.sleep como si
importó sleep directamente.
"""

import contextlib
import io
import sys
import time
import unittest
from unittest import mock

import reintentos
from reintentos import reintentar


@contextlib.contextmanager
def esperas_interceptadas():
    """Devuelve la lista donde quedan apuntadas las esperas, sin dormir ninguna."""
    esperas = []
    parches = [mock.patch("time.sleep", side_effect=esperas.append)]
    if hasattr(reintentos, "sleep"):
        parches.append(mock.patch.object(reintentos, "sleep", side_effect=esperas.append))
    with contextlib.ExitStack() as pila:
        for parche in parches:
            pila.enter_context(parche)
        yield esperas


class Fallo(Exception):
    pass


def conserva_nombre_y_docstring():
    @reintentar(intentos=1)
    def funcion_documentada():
        """Documentación original."""
        return 1

    return (
        funcion_documentada.__name__ == "funcion_documentada"
        and funcion_documentada.__doc__ == "Documentación original."
    )


def devuelve_el_valor_tras_fallar():
    llamadas = []

    @reintentar(intentos=3, espera_inicial=0.01, excepciones=(Fallo,), jitter=False)
    def falla_dos_veces():
        llamadas.append(1)
        if len(llamadas) < 3:
            raise Fallo()
        return 42

    with esperas_interceptadas():
        return falla_dos_veces() == 42 and len(llamadas) == 3


def funciona_sin_valor_de_retorno():
    @reintentar(intentos=2)
    def no_devuelve():
        pass

    with esperas_interceptadas():
        return no_devuelve() is None


def relanza_la_ultima_excepcion():
    lanzadas = []

    @reintentar(intentos=3, espera_inicial=0.01, excepciones=(Fallo,), jitter=False)
    def siempre_falla():
        excepcion = Fallo(f"intento {len(lanzadas) + 1}")
        lanzadas.append(excepcion)
        raise excepcion

    with esperas_interceptadas():
        try:
            siempre_falla()
        except Fallo as capturada:
            return len(lanzadas) == 3 and capturada is lanzadas[-1]
    return False


def no_reintenta_excepciones_no_listadas():
    llamadas = []

    @reintentar(intentos=3, espera_inicial=0.01, excepciones=(ValueError,))
    def lanza_otra():
        llamadas.append(1)
        raise KeyError("otra familia")

    with esperas_interceptadas():
        try:
            lanza_otra()
        except KeyError:
            return len(llamadas) == 1
    return False


def la_espera_es_exponencial_sin_jitter():
    @reintentar(intentos=4, espera_inicial=0.1, excepciones=(Fallo,), jitter=False)
    def siempre_falla():
        raise Fallo()

    with esperas_interceptadas() as esperas:
        try:
            siempre_falla()
        except Fallo:
            pass
    return [round(e, 4) for e in esperas] == [0.1, 0.2, 0.4]


def el_jitter_cambia_las_esperas():
    """Con jitter, en cinco rondas al menos una espera tiene que salirse de la serie exacta."""
    exactas = [0.1, 0.2, 0.4]
    distintas = False
    for _ in range(5):

        @reintentar(intentos=4, espera_inicial=0.1, excepciones=(Fallo,), jitter=True)
        def siempre_falla():
            raise Fallo()

        with esperas_interceptadas() as esperas:
            try:
                siempre_falla()
            except Fallo:
                pass
        if len(esperas) == 3 and [round(e, 6) for e in esperas] != exactas:
            distintas = True
    return distintas


def intentos_cero_no_produce_un_typeerror():
    """Con intentos=0, «raise None» da un TypeError que no dice nada. Cualquier otra cosa vale:
    rechazar el argumento al decorar, o no llamar a la función."""
    try:

        @reintentar(intentos=0)
        def nunca():
            raise Fallo()

        with esperas_interceptadas():
            nunca()
    except TypeError:
        return False
    except Exception:
        return True
    return True


def sus_pruebas_pasan_sin_dormir():
    """El enunciado pide pruebas con unittest que no tarden segundos reales."""
    suite = unittest.defaultTestLoader.loadTestsFromModule(reintentos)
    if suite.countTestCases() == 0:
        return False
    inicio = time.monotonic()
    resultado = unittest.TextTestRunner(stream=io.StringIO(), verbosity=0).run(suite)
    transcurrido = time.monotonic() - inicio
    print(f"sus pruebas: {suite.countTestCases()}, {transcurrido:.3f} s")
    return resultado.wasSuccessful() and transcurrido < 1.0


PRUEBAS = {
    funcion.__name__: funcion
    for funcion in [
        conserva_nombre_y_docstring,
        devuelve_el_valor_tras_fallar,
        funciona_sin_valor_de_retorno,
        relanza_la_ultima_excepcion,
        no_reintenta_excepciones_no_listadas,
        la_espera_es_exponencial_sin_jitter,
        el_jitter_cambia_las_esperas,
        intentos_cero_no_produce_un_typeerror,
        sus_pruebas_pasan_sin_dormir,
    ]
}


def main():
    nombre = sys.argv[1] if len(sys.argv) > 1 else ""
    prueba = PRUEBAS.get(nombre)
    if prueba is None:
        print(f"no existe la prueba «{nombre}»", file=sys.stderr)
        return 2
    try:
        pasa = prueba() is True
    except Exception as fallo:  # una excepción inesperada es un fallo de la entrega, no del banco
        print(f"lanzó {type(fallo).__name__}: {fallo}", file=sys.stderr)
        pasa = False
    print(f"{'PASA ' if pasa else 'FALLA'}  {nombre}")
    return 0 if pasa else 1


if __name__ == "__main__":
    sys.exit(main())
