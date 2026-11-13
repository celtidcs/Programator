# U03: cuota mensual de un préstamo en Python

Corresponde a la fila de desempeño «Función acotada». Se usa solo cuando las pruebas ocultas no
dieron nota (apagadas, o sin Python en el equipo). Se puntúa **leyendo el código, sin ejecutarlo**,
con lo mismo que miran las pruebas: la firma, la fórmula, el caso sin intereses, el redondeo y los
tres rechazos.

La función de referencia, para comparar:

```python
def cuota_mensual(capital: float, interes_anual: float, meses: int) -> float:
    if capital <= 0 or meses <= 0 or interes_anual < 0:
        raise ValueError(...)
    if interes_anual == 0:
        return round(capital / meses, 2)
    i = interes_anual / 100 / 12
    return round(capital * i / (1 - (1 + i) ** -meses), 2)
```

Con ella, `cuota_mensual(10000, 5, 12)` da 856,07, `(200000, 3, 360)` da 843,21,
`(1200, 0, 12)` da 100,00, `(5000, 7.5, 60)` da 100,19 y `(1, 12, 1)` da 1,01.

## Qué se mira, y cuánto vale

Suma los puntos de lo que se cumple. Cada punto se da entero o no se da, salvo donde se dice
otra cosa.

- **0,5**: la firma es exactamente `def cuota_mensual(capital: float, interes_anual: float,
  meses: int) -> float`.
- **1,5**: la fórmula es la del enunciado, con el interés mensual en tanto por uno: el anual
  **entre 100 y entre 12**. Olvidar el `/ 100` (tratar 5 como un 500 %) o el `/ 12` es el fallo
  típico y no se da este punto.
- **1**: con `interes_anual == 0` devuelve `capital / meses`, sin dividir por cero en la fórmula.
- **0,5**: redondea el resultado a 2 decimales con `round(..., 2)`, en los dos caminos.
- **1**: lanza `ValueError` si `capital <= 0`, si `meses <= 0` o si `interes_anual < 0`, antes de
  calcular nada (0,5 si falta uno de los tres; 0 si faltan dos o lanza otra excepción).
- **0,5**: solo biblioteca estándar: nada de `numpy`, `numpy_financial` ni similares.

## Qué baja la nota

- **0**, como con las pruebas: no entregó código, la función se llama de otra forma o tiene un
  error que con seguridad impide importar el módulo. Si solo dudas, no restes por eso y dilo en
  los motivos.
- **Máximo 1**: usa otra fórmula (amortización alemana, interés simple) aunque el resultado se
  parezca.

Una nota de esta rúbrica no es comparable con la de las pruebas ocultas: el informe dice cuál es
cuál.
