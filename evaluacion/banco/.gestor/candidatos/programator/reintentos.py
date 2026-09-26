import functools
import random
import time
from typing import Callable, TypeVar, Type, Any

T = TypeVar('T')


def reintentar(
    intentos: int = 3,
    espera_inicial: float = 0.1,
    excepciones: tuple[Type[Exception], ...] = (Exception,),
    jitter: bool = True
) -> Callable[[Callable[..., T]], Callable[..., T]]:
    """
    Decorador que reintenta una función con espera exponencial.
    
    Args:
        intentos: Número máximo de intentos.
        espera_inicial: Tiempo inicial de espera en segundos.
        excepciones: Tupla de excepciones que activan el reintento.
        jitter: Si True, añade un factor aleatorio al tiempo de espera.
    
    Returns:
        Decorador que aplica la lógica de reintento.
    """
    def decorador(func: Callable[..., T]) -> Callable[..., T]:
        @functools.wraps(func)
        def wrapper(*args: Any, **kwargs: Any) -> T:
            ultima_excepcion = None
            for intento in range(intentos):
                try:
                    return func(*args, **kwargs)
                except excepciones as e:
                    ultima_excepcion = e
                    if intento < intentos - 1:
                        espera = espera_inicial * (2 ** intento)
                        if jitter:
                            espera *= random.uniform(0.5, 1.5)
                        time.sleep(espera)
            raise ultima_excepcion
        return wrapper
    return decorador


# Pruebas con unittest
import unittest


class TestReintentar(unittest.TestCase):
    def test_funcion_exitosa(self):
        @reintentar(intentos=3, espera_inicial=0.1)
        def funcion_exitosa():
            return "éxito"
        self.assertEqual(funcion_exitosa(), "éxito")

    def test_funcion_con_error_temporal(self):
        contador = 0

        @reintentar(intentos=3, espera_inicial=0.1, excepciones=(ValueError,))
        def funcion_con_error():
            nonlocal contador
            contador += 1
            if contador < 3:
                raise ValueError("Error temporal")
            return "éxito"
        self.assertEqual(funcion_con_error(), "éxito")

    def test_funcion_con_error_persistente(self):
        @reintentar(intentos=3, espera_inicial=0.1, excepciones=(ValueError,))
        def funcion_con_error_persistente():
            raise ValueError("Error persistente")
        with self.assertRaises(ValueError) as context:
            funcion_con_error_persistente()
        self.assertEqual(str(context.exception), "Error persistente")

    def test_funcion_sin_retorno(self):
        contador = 0

        @reintentar(intentos=3, espera_inicial=0.1, excepciones=(ValueError,))
        def funcion_sin_retorno():
            nonlocal contador
            contador += 1
            if contador < 3:
                raise ValueError("Error temporal")
        try:
            funcion_sin_retorno()
        except ValueError:
            self.fail("La función no debería lanzar una excepción")

    def test_jitter_desactivado(self):
        contador = 0
        tiempos_espera = []

        @reintentar(intentos=3, espera_inicial=0.1, excepciones=(ValueError,), jitter=False)
        def funcion_con_jitter_desactivado():
            nonlocal contador
            contador += 1
            if contador < 3:
                raise ValueError("Error temporal")
            return "éxito"
        inicio = time.time()
        funcion_con_jitter_desactivado()
        fin = time.time()
        tiempo_transcurrido = fin - inicio
        self.assertGreaterEqual(tiempo_transcurrido, 0.3)
        self.assertLess(tiempo_transcurrido, 0.4)


if __name__ == "__main__":
    unittest.main()