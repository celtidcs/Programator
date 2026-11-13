Encargo de codigo. Entregalo con escribir_propuesta, nombre exacto config.ts

En TypeScript con strict activado, escribe un validador de configuracion:

- Exporta exactamente este tipo, como union discriminada, sin excepciones:
  export type Resultado<T, E> = { ok: true; valor: T } | { ok: false; error: E };
- export interface Config { puerto: number; host: string; reintentos: number; depurar?: boolean }
- export function parsearConfig(entrada: unknown): Resultado<Config, string[]>
  que valide un objeto desconocido y acumule TODOS los errores, no solo el primero.
- puerto entre 1 y 65535. reintentos >= 0 y entero. host no vacio. depurar, si viene, booleano.

Prohibido: any, y el operador as salvo que expliques en un comentario por que es seguro.
Incluye al final tres ejemplos de uso comentados: uno valido y dos invalidos.

Publica despues un resumen de lo entregado.
