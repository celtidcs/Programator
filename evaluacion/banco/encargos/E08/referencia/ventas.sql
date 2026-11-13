WITH totales AS (
    SELECT c.pais, c.id AS cliente_id, c.nombre, SUM(v.importe) AS total
    FROM ventas v
    JOIN clientes c ON c.id = v.cliente_id
    WHERE v.fecha >= '2025-01-01' AND v.fecha < '2026-01-01'
    GROUP BY c.pais, c.id, c.nombre
),
clasificados AS (
    SELECT pais, cliente_id, nombre, total,
           ROW_NUMBER() OVER (PARTITION BY pais ORDER BY total DESC, cliente_id) AS posicion,
           ROUND(100.0 * total / SUM(total) OVER (PARTITION BY pais), 2) AS porcentaje
    FROM totales
)
SELECT pais, cliente_id, nombre, total, posicion, porcentaje
FROM clasificados
WHERE posicion <= 3
ORDER BY pais, posicion;

-- Índice: la consulta filtra ventas por fecha y agrupa por cliente, así que un índice compuesto
-- por fecha y cliente permite recorrer solo el año pedido y leer el cliente sin ir a la tabla.
-- CREATE INDEX idx_ventas_fecha_cliente ON ventas (fecha, cliente_id, importe);
