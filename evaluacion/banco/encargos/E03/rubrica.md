# E03: expandir una cadena de rangos en Rust

Se usa solo cuando las pruebas ocultas no dieron nota (apagadas, o sin Rust en el equipo). Se
puntúa **leyendo el código, sin ejecutarlo**, con lo mismo que miran las pruebas: la firma, los
casos del enunciado, los cuatro rechazos y que nada entre en pánico.

## Qué se mira, y cuánto vale

Suma los puntos de lo que se cumple. Cada punto se da entero o no se da.

- **1**: la firma es exactamente `pub fn expandir(entrada: &str) -> Result<Vec<u32>, ErrorRango>`,
  `ErrorRango` es un `enum` propio y el fichero es el contenido de un módulo, sin envolverlo en
  otro `mod`.
- **1**: expande bien lo válido: `"1-3,7,10-12"` da `[1,2,3,7,10,11,12]` (rangos con los dos
  extremos incluidos), un número suelto (`"7"`) da `[7]` y un rango de un elemento (`"5-5"`) da
  `[5]`.
- **0,5**: rechaza la cadena vacía y los separadores sobrantes: coma al principio (`",1"`), al final
  (`"1,2,"`) y doble (`"1,,2"`). Lo normal es que cualquier fragmento vacío sea un error.
- **0,5**: rechaza el rango invertido (`"5-2"`).
- **0,5**: rechaza un número no válido (`"abc"`, `"1-x"`) y uno que no cabe en `u32`
  (`"4294967296"`), devolviendo un error: el error de `parse::<u32>()` se convierte en una variante
  de `ErrorRango`, no se desenvuelve.
- **0,5**: el error dice dónde falló: la variante lleva el fragmento que falló o su posición.
- **0,5**: no hay `unwrap()`, `expect()`, `panic!` ni nada que pueda entrar en pánico antes de
  `#[cfg(test)]`: ni índices sin comprobar ni `as` que trunque.
- **0,5**: hay un módulo `#[cfg(test)]` con al menos cinco `#[test]`, incluidos casos de error, y
  leídos parecen correctos.

## Qué baja la nota

- **0**, como con las pruebas: no entregó código, la firma es otra (otro nombre, otros tipos, no es
  `pub`) o tiene un error que con seguridad impide compilar (sintaxis rota, un tipo o un método
  que no existe). Si solo dudas de si compila, no restes por eso y dilo en los motivos.
- **Máximo 2**: usa una biblioteca externa (un `crate` que no sea `std`) o reescribe la tarea en
  algo que no es lo pedido.

Una nota de esta rúbrica no es comparable con la de las pruebas ocultas: el informe dice cuál es
cuál.
