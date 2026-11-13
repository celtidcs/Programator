export type Resultado<T, E> = { ok: true; valor: T } | { ok: false; error: E };

export interface Config {
  puerto: number;
  host: string;
  reintentos: number;
  depurar?: boolean;
}

function esRegistro(valor: unknown): valor is Record<string, unknown> {
  return typeof valor === 'object' && valor !== null && !Array.isArray(valor);
}

export function parsearConfig(entrada: unknown): Resultado<Config, string[]> {
  if (!esRegistro(entrada)) {
    return { ok: false, error: ['la configuración debe ser un objeto'] };
  }
  const errores: string[] = [];
  const { puerto, host, reintentos, depurar } = entrada;

  if (typeof puerto !== 'number' || !Number.isInteger(puerto) || puerto < 1 || puerto > 65535) {
    errores.push('puerto debe ser un entero entre 1 y 65535');
  }
  if (typeof host !== 'string' || host.length === 0) {
    errores.push('host no puede estar vacío');
  }
  if (typeof reintentos !== 'number' || !Number.isInteger(reintentos) || reintentos < 0) {
    errores.push('reintentos debe ser un entero mayor o igual que 0');
  }
  if (depurar !== undefined && typeof depurar !== 'boolean') {
    errores.push('depurar, si viene, debe ser booleano');
  }

  if (
    errores.length > 0 ||
    typeof puerto !== 'number' ||
    typeof host !== 'string' ||
    typeof reintentos !== 'number'
  ) {
    return { ok: false, error: errores };
  }
  const config: Config = { puerto, host, reintentos };
  if (typeof depurar === 'boolean') {
    config.depurar = depurar;
  }
  return { ok: true, valor: config };
}

// Ejemplos:
// parsearConfig({ puerto: 80, host: 'a', reintentos: 0 })   -> { ok: true, ... }
// parsearConfig({ puerto: 0, host: '', reintentos: -1 })    -> tres errores
// parsearConfig([])                                         -> no es un objeto
