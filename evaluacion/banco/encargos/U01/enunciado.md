Encargo de auditoria. No escribas codigo ni entregues fichero: publica los hallazgos en tu buzon.

Revisa el fichero Pedidos.cs, en la raiz de la carpeta de trabajo, buscando problemas de forma:
numeros o textos sueltos que deberian ser constantes con nombre, metodos demasiado largos, clases
que hacen mas de una cosa y codigo duplicado.

Usa este formato exacto, una linea por hallazgo, con seis campos separados por |:

    TIPO | fichero | miembro | linea | literal | consecuencia

- TIPO: HARDCODEO, DUPLICADO, TAMAÑO o SOLID.
- fichero: Pedidos.cs
- miembro: el metodo o la clase donde esta.
- linea: el numero de linea.
- literal: un fragmento exacto del fichero que justifique el hallazgo y que aparezca una sola vez.
- consecuencia: que harias, en pocas palabras.

Si una parte del fichero esta bien, no la menciones. No rellenes: un hallazgo que no existe cuesta
mas que uno que se te escapa.
