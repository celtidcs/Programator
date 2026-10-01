# Formato formal de auditoría, con línea verificada

Pega este fichero junto al encargo cuando quieras que el arnés compruebe por sí mismo la línea que
cites en cada hallazgo, en vez de confiar en lo que digas tú.

## El formato, exacto

Una línea por hallazgo, con seis campos separados por `|`:

```
TIPO | fichero | miembro | línea | literal | consecuencia
```

- **TIPO**: una de estas cuatro palabras, en mayúsculas: `HARDCODEO`, `DUPLICADO`, `TAMAÑO`,
  `SOLID`.
- **fichero**: la ruta del fichero donde está el hallazgo, tal como se te dio en el encargo.
- **miembro**: el método, propiedad o clase donde está.
- **línea**: el número de línea donde crees que está, **tal cual lo recuerdes**. No hace falta que
  sea exacto: el arnés lo comprueba y lo corrige si hace falta.
- **literal**: el fragmento de texto EXACTO, tal como aparece en el fichero, que justifica el
  hallazgo (por ejemplo, `380f`, o el nombre de una variable). Es la clave de todo: sin un literal
  que exista de verdad en el fichero, el arnés no puede comprobar nada.
- **consecuencia**: qué harías con ese hallazgo, en pocas palabras.

## Qué hace el arnés con esto

Antes de que tu respuesta llegue al canal, el arnés busca cada `literal` en el fichero que nombras.

Si lo encuentra en una única línea, sustituye tu número por el real y, si eran distintos, lo deja
anotado («corregida; el modelo dijo N»), para que quien lo lea sepa que hubo un ajuste.

Si el literal aparece más de una vez en el fichero, o no aparece, **no adivina**: deja tu número tal
cual y añade «sin verificar automáticamente». Eso significa que tienes que elegir un literal que
aparezca una sola vez, o que la persona que revise sabrá que esa línea concreta no se pudo
confirmar sola.

## Un ejemplo

Si escribes:

```
HARDCODEO | game/scripts/Mapa.cs | CalcularTamano | 200 | 380f | darle nombre con una constante
```

y `380f` está de verdad en la línea 106 del fichero, y en ninguna otra, lo que llega al canal es:

```
HARDCODEO | game/scripts/Mapa.cs | CalcularTamano | 106 (corregida; el modelo dijo 200) | 380f | darle nombre con una constante
```

## Lo que no cubre

Ningún campo puede contener `|`: si tu literal es un fragmento de código con una barra vertical
dentro (una consulta SQL, por ejemplo), esa línea no encajará en el formato y no se corregirá. Elige
un fragmento más corto que no la lleve.

Esto no arregla que el hallazgo en sí esté bien fundado —si marcas como hardcodeo algo que ya tiene
nombre, sigue siendo un error tuyo—; solo arregla la línea que citas, una vez decidido qué decir.
