local Cola = {}
Cola.__index = Cola

function Cola.nueva(capacidad)
  assert(type(capacidad) == "number" and capacidad >= 1, "capacidad debe ser un número mayor que 0")
  return setmetatable({ elementos = {}, cabeza = 1, final = 0, capacidad = capacidad }, Cola)
end

function Cola:tamano()
  return self.final - self.cabeza + 1
end

function Cola:meter(valor)
  if self:tamano() >= self.capacidad then
    return false
  end
  self.final = self.final + 1
  self.elementos[self.final] = valor
  return true
end

function Cola:sacar()
  if self:tamano() == 0 then
    return nil
  end
  local valor = self.elementos[self.cabeza]
  self.elementos[self.cabeza] = nil
  self.cabeza = self.cabeza + 1
  return valor
end

return Cola
