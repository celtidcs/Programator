Encargo de diagnostico. No hace falta que entregues fichero: publica la respuesta en tu buzon.

Este inventario en Lua 5.4 funciona bien hasta que se quita un producto. A partir de ahi el
informe deja de mostrar productos que siguen en el inventario, y los que se anaden despues a veces
aparecen y a veces no.

    local Inventario = {}
    Inventario.__index = Inventario

    function Inventario.nuevo()
      return setmetatable({ productos = {} }, Inventario)
    end

    function Inventario:anadir(nombre, unidades)
      self.productos[#self.productos + 1] = { nombre = nombre, unidades = unidades }
    end

    function Inventario:quitar(nombre)
      for i, p in ipairs(self.productos) do
        if p.nombre == nombre then
          self.productos[i] = nil
          return true
        end
      end
      return false
    end

    function Inventario:informe()
      local lineas = {}
      for _, p in ipairs(self.productos) do
        lineas[#lineas + 1] = p.nombre .. ": " .. p.unidades
      end
      return table.concat(lineas, "\n")
    end

    return Inventario

Lo que se observa:

    local inv = Inventario.nuevo()
    inv:anadir("tornillos", 100)
    inv:anadir("tuercas", 50)
    inv:anadir("arandelas", 75)
    inv:quitar("tuercas")
    print(inv:informe())   -- imprime solo "tornillos: 100"

Explica la causa exacta, por que lo que se anade despues a veces aparece y a veces no, y como se
arregla.
