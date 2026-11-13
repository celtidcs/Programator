Encargo de codigo. Entregalo con la herramienta escribir_propuesta, con el nombre exacto rangos.rs

Escribe en Rust una funcion publica que convierta una cadena de rangos en una lista de numeros:

    "1-3,7,10-12"  ->  [1,2,3,7,10,11,12]

Requisitos, todos obligatorios:
- Firma: pub fn expandir(entrada: &str) -> Result<Vec<u32>, ErrorRango>
- ErrorRango es un enum propio con variantes que digan que fallo y donde.
- Prohibido unwrap() y expect() en el codigo de produccion.
- Rechaza: cadena vacia, rango invertido (5-2), numero no valido, y separadores sobrantes.
- Incluye un modulo de pruebas con al menos cinco casos, incluidos los de error.
- El fichero rangos.rs es un modulo de Rust: escribe su contenido directamente, sin envolverlo en
  otro mod.

Cuando termines, publica un resumen de que has entregado y como se comprueba.
