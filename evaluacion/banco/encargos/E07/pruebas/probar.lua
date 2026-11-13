-- Pruebas que el modelo no ve, sobre su cola.lua.
-- Uso: lua5.4 probar.lua <nombre>. Termina con 0 si pasa, 1 si falla y 2 si el nombre no existe.

package.path = "./?.lua;" .. package.path

-- Las globales que existen antes de cargar el módulo, para saber después si creó alguna.
local globales_previas = {}
for clave in pairs(_G) do globales_previas[clave] = true end

local Cola = require("cola")

local function secuencia(c, pasos)
  local r = {}
  for _, paso in ipairs(pasos) do
    if paso[1] == "meter" then r[#r + 1] = tostring(c:meter(paso[2]))
    elseif paso[1] == "sacar" then r[#r + 1] = tostring(c:sacar())
    else r[#r + 1] = tostring(c:tamano()) end
  end
  return table.concat(r, ",")
end

local pruebas = {}

function pruebas.comportamiento_fifo_con_limite()
  local c = Cola.nueva(2)
  local obtenido = secuencia(c, {
    {"meter", "a"}, {"meter", "b"}, {"meter", "c"}, {"tamano"},
    {"sacar"}, {"sacar"}, {"sacar"}, {"tamano"},
  })
  return obtenido == "true,true,false,2,a,b,nil,0", obtenido
end

function pruebas.se_reutiliza_tras_vaciarse()
  local c = Cola.nueva(2)
  for vuelta = 1, 3 do
    if not c:meter(vuelta) or not c:meter(vuelta * 10) then return false, "no admite tras vaciar" end
    if c:sacar() ~= vuelta or c:sacar() ~= vuelta * 10 or c:sacar() ~= nil then
      return false, "orden incorrecto en la vuelta " .. vuelta
    end
  end
  return true
end

function pruebas.dos_colas_son_independientes()
  local a, b = Cola.nueva(3), Cola.nueva(3)
  a:meter("x")
  return a:tamano() == 1 and b:tamano() == 0 and b:sacar() == nil
end

function pruebas.las_instancias_usan_metatabla()
  local c = Cola.nueva(1)
  return getmetatable(c) ~= nil
end

function pruebas.no_crea_variables_globales()
  local c = Cola.nueva(2)
  c:meter(1); c:sacar(); c:tamano()
  local nuevas = {}
  for clave in pairs(_G) do
    if not globales_previas[clave] and clave ~= "Cola" then nuevas[#nuevas + 1] = tostring(clave) end
  end
  if rawget(_G, "Cola") ~= nil then nuevas[#nuevas + 1] = "Cola" end
  return #nuevas == 0, table.concat(nuevas, ", ")
end

-- Con table.remove(t, 1), vaciar cien mil elementos cuesta del orden de cinco mil millones de
-- movimientos; con índices de cabeza y cola, unos pocos milisegundos. El límite deja mucho margen.
function pruebas.meter_y_sacar_son_de_coste_constante()
  local n = 100000
  local c = Cola.nueva(n)
  local inicio = os.clock()
  for i = 1, n do c:meter(i) end
  for i = 1, n do
    if c:sacar() ~= i then return false, "orden incorrecto en " .. i end
  end
  local segundos = os.clock() - inicio
  return segundos < 2.0, string.format("%.3f s", segundos)
end

local nombre = arg[1] or ""
local prueba = pruebas[nombre]
if not prueba then
  io.stderr:write("no existe la prueba «" .. nombre .. "»\n")
  os.exit(2)
end
local ok, pasa, detalle = pcall(prueba)
if not ok then
  detalle, pasa = tostring(pasa), false
end
print((pasa and "PASA " or "FALLA") .. "  " .. nombre .. (detalle and ("  " .. tostring(detalle)) or ""))
os.exit(pasa and 0 or 1)
