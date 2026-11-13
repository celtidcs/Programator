"""Cuota mensual de un préstamo con amortización francesa."""

MESES_POR_ANO = 12
PORCENTAJE = 100
DECIMALES = 2


def cuota_mensual(capital: float, interes_anual: float, meses: int) -> float:
    if capital <= 0:
        raise ValueError("el capital debe ser positivo")
    if meses <= 0:
        raise ValueError("los meses deben ser positivos")
    if interes_anual < 0:
        raise ValueError("el interés no puede ser negativo")
    if interes_anual == 0:
        return round(capital / meses, DECIMALES)
    i = interes_anual / PORCENTAJE / MESES_POR_ANO
    return round(capital * i / (1 - (1 + i) ** -meses), DECIMALES)
