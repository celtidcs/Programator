"""Decorador de reintentos con espera exponencial y jitter."""

import functools
import random
import time
import unittest
from unittest import mock


def reintentar(intentos=3, espera_inicial=0.1, excepciones=(Exception,), jitter=True):
    """Reintenta la función decorada hasta `intentos` veces con espera exponencial.

    Si se agotan los intentos, relanza la última excepción capturada.
    """
    if intentos < 1:
        raise ValueError("intentos debe ser al menos 1")

    def decorador(funcion):
        @functools.wraps(funcion)
        def envoltorio(*args, **kwargs):
            espera = espera_inicial
            for intento in range(1, intentos + 1):
                try:
                    return funcion(*args, **kwargs)
                except excepciones:
                    if intento == intentos:
                        raise
                    time.sleep(espera * random.uniform(0.5, 1.5) if jitter else espera)
                    espera *= 2
            return None

        return envoltorio

    return decorador


class PruebasReintentar(unittest.TestCase):
    def test_devuelve_valor(self):
        llamadas = []

        @reintentar(intentos=3, jitter=False)
        def falla_una_vez():
            llamadas.append(1)
            if len(llamadas) == 1:
                raise ValueError()
            return "ok"

        with mock.patch("time.sleep"):
            self.assertEqual(falla_una_vez(), "ok")

    def test_relanza_la_ultima(self):
        @reintentar(intentos=2, jitter=False)
        def siempre():
            raise KeyError("x")

        with mock.patch("time.sleep"):
            with self.assertRaises(KeyError):
                siempre()

    def test_conserva_nombre(self):
        @reintentar()
        def f():
            """doc"""

        self.assertEqual((f.__name__, f.__doc__), ("f", "doc"))


if __name__ == "__main__":
    unittest.main()
