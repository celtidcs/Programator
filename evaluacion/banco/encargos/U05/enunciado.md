Encargo de auditoria. No escribas codigo ni entregues fichero: publica los hallazgos en tu buzon.

Antes de dar por bueno el fichero texto.rs, en la raiz de la carpeta de trabajo, quiero una segunda
opinion. Revisalo buscando problemas de forma: numeros o textos sueltos que deberian ser constantes
con nombre, funciones demasiado largas, tipos que hacen mas de una cosa y codigo duplicado.

Usa este formato exacto, una linea por hallazgo, con seis campos separados por |:

    TIPO | fichero | miembro | linea | literal | consecuencia

- TIPO: HARDCODEO, DUPLICADO, TAMAÑO o SOLID.
- fichero: texto.rs
- miembro: la funcion o el tipo donde esta.
- linea: el numero de linea.
- literal: un fragmento exacto del fichero que justifique el hallazgo y que aparezca una sola vez.
- consecuencia: que harias, en pocas palabras.

Si no encuentras nada, dilo con esas palabras. No rellenes: un hallazgo que no existe cuesta mas
que uno que se te escapa.
