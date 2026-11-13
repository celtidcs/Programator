Encargo de codigo. Entregalo con escribir_propuesta, nombre exacto cola.lua

En Lua 5.4, un modulo que implemente una cola FIFO con limite de tamano:
- Cola.nueva(capacidad) devuelve una instancia; usa metatablas, no closures.
- metodos: :meter(v) devuelve true si cabe y false si esta llena, :sacar() devuelve nil si esta
  vacia, :tamano()
- La cola debe ser O(1) en meter y sacar: no uses table.remove(t, 1).
- Nada de variables globales.
- El fichero termina con return Cola, para cargarlo con require.
