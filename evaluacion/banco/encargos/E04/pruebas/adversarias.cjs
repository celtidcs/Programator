// Pruebas que el modelo no ve, sobre su config.ts ya compilado a build/config.js.
// Uso: node adversarias.cjs <nombre>. Termina con 0 si pasa, 1 si falla y 2 si el nombre no existe.

const { parsearConfig } = require('./build/config.js');

const valida = { puerto: 8080, host: 'localhost', reintentos: 3 };
const rechaza = (entrada) => parsearConfig(entrada).ok === false;

const pruebas = {
  acepta_una_configuracion_valida: () => {
    const r = parsearConfig(valida);
    return r.ok === true && r.valor.puerto === 8080 && r.valor.host === 'localhost' && r.valor.reintentos === 3;
  },
  acepta_depurar_cuando_es_booleano: () => parsearConfig({ ...valida, depurar: true }).ok === true,
  acumula_todos_los_errores: () => {
    // Tres campos mal a la vez: tienen que salir al menos tres errores, no solo el primero.
    const r = parsearConfig({ puerto: 0, host: '', reintentos: -1 });
    return r.ok === false && Array.isArray(r.error) && r.error.length >= 3;
  },
  rechaza_puertos_fuera_de_rango: () =>
    rechaza({ ...valida, puerto: 0 }) && rechaza({ ...valida, puerto: 65536 }),
  // NaN es de tipo number en JavaScript, y no está entre 1 y 65535.
  rechaza_puerto_nan: () => rechaza({ ...valida, puerto: NaN }),
  rechaza_reintentos_negativos_o_no_enteros: () =>
    rechaza({ ...valida, reintentos: -1 }) &&
    rechaza({ ...valida, reintentos: 1.5 }) &&
    rechaza({ ...valida, reintentos: Infinity }),
  rechaza_host_vacio: () => rechaza({ ...valida, host: '' }),
  rechaza_depurar_que_no_es_booleano: () => rechaza({ ...valida, depurar: 'si' }),
  rechaza_lo_que_no_es_un_objeto: () =>
    rechaza(null) && rechaza(undefined) && rechaza('puerto=8080') && rechaza(42),
  // typeof [] === 'object': un validador que solo mira typeof deja pasar un array.
  rechaza_un_array: () => rechaza([]),
};

const nombre = process.argv[2];
const prueba = pruebas[nombre];
if (!prueba) {
  console.error(`no existe la prueba «${nombre}»`);
  process.exit(2);
}
let pasa = false;
try {
  pasa = prueba() === true;
} catch (fallo) {
  console.error(`lanzó una excepción: ${fallo}`);
}
console.log(`${pasa ? 'PASA ' : 'FALLA'}  ${nombre}`);
process.exit(pasa ? 0 : 1);
