Encargo de auditoria. No escribas codigo ni entregues fichero: publica los hallazgos en tu buzon.

Revisa el fichero reservas.rs, en la raiz de la carpeta de trabajo, buscando dos clases de
problema: validaciones que faltan (entradas que deberian rechazarse y se aceptan) y errores de
significado (codigo que hace algo distinto de lo que dicen sus comentarios o su nombre). No busques
problemas de estilo ni de forma.

Usa este formato exacto, una linea por hallazgo, con seis campos separados por |:

    TIPO | fichero | miembro | linea | literal | consecuencia

- TIPO: VALIDACION si falta una comprobacion, SIGNIFICADO si el codigo hace otra cosa de la que
  dice.
- fichero: reservas.rs
- miembro: la funcion donde esta.
- linea: el numero de linea.
- literal: un fragmento exacto del fichero que justifique el hallazgo.
- consecuencia: que entrada lo provoca y que pasa, en pocas palabras.

No rellenes: un hallazgo que no existe cuesta mas que uno que se te escapa.
