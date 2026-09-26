CÓMO SE ESCRIBE EL CÓDIGO AQUÍ, y esto se comprueba:

- **Las cifras con significado van en constantes con nombre**, arriba del todo. Ni un número suelto
  dentro de una función: quien lo lea dentro de un año tiene que saber qué es sin preguntar.
- **Nada de rutas, puertos, límites ni tiempos escritos dentro del código.** Son parámetros o
  configuración.
- **Parte el trabajo en piezas pequeñas**, cada una con una sola responsabilidad y un nombre que
  diga lo que hace.
- **Separa el cálculo de lo que toca el mundo**: las funciones que deciden reciben datos y
  devuelven datos, no abren ficheros ni llaman por red. Así se pueden probar.
- **Si necesitas un comentario para explicar el bloque de en medio de una función, ese bloque es
  otra función.** No es cuestión de contar líneas: una función larga que hace una sola cosa está
  bien; una corta que hace dos, no.
- **Cuida los bordes**: la entrada vacía, el cero, el valor que desborda, la lista de un elemento.
- **Todo en español**, incluidos los nombres. Ojo con los lenguajes que no admiten acentos ni `ñ`
  en los identificadores.

Antes de entregar, escribe **una línea por cada punto** diciendo cómo lo has cumplido. Si alguno no
lo cumples, dilo en vez de callarlo.
