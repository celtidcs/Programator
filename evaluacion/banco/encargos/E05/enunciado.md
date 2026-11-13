Encargo de codigo. Entregalo con escribir_propuesta, nombre exacto reintentos.py

Escribe un decorador Python que reintente una funcion con espera exponencial:

- Firma: def reintentar(intentos=3, espera_inicial=0.1, excepciones=(Exception,), jitter=True)
- Espera exponencial: 0.1, 0.2, 0.4... con jitter aleatorio para no sincronizar clientes.
- Debe conservar el nombre y el docstring de la funcion decorada.
- Si se agotan los intentos, relanza la ultima excepcion, no una nueva.
- Debe funcionar con funciones que devuelven valores y con las que no.
- Escribe pruebas con unittest en el mismo fichero, que no tarden segundos reales en ejecutarse.

Publica un resumen de lo entregado.
