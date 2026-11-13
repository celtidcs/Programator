Encargo de codigo. Entregalo con escribir_propuesta, nombre exacto cuota.py

Escribe en Python la funcion que calcula la cuota mensual de un prestamo con amortizacion
francesa (cuota constante).

Requisitos, todos obligatorios:
1. Firma exacta: def cuota_mensual(capital: float, interes_anual: float, meses: int) -> float
2. interes_anual es un porcentaje: 5 significa un 5 % anual. El interes mensual es el anual entre 12.
3. Formula: cuota = capital * i / (1 - (1 + i) ** -meses), con i el interes mensual en tanto por uno.
4. Si interes_anual es 0, la cuota es capital / meses.
5. El resultado se redondea a 2 decimales con round.
6. Lanza ValueError si capital <= 0, si meses <= 0 o si interes_anual < 0.
7. Solo biblioteca estandar.

Valor de referencia para que compruebes tu resultado: cuota_mensual(10000, 5, 12) da 856.07.
