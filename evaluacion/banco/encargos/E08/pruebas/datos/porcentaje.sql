-- El porcentaje es sobre el total del país en 2025 (1000), no sobre la suma de los tres primeros
-- (900) ni sobre el total de todos los países (1100).
-- DE: Karl 400, Lena 300, Max 200, Nora 100; total 1000. IT: Olga 100.
INSERT INTO clientes VALUES
    (1, 'Karl', 'DE'), (2, 'Lena', 'DE'), (3, 'Max', 'DE'), (4, 'Nora', 'DE'), (5, 'Olga', 'IT');
INSERT INTO ventas (cliente_id, importe, fecha) VALUES
    (1, 400, '2025-01-01'), (2, 300, '2025-02-01'), (3, 200, '2025-03-01'),
    (4, 100, '2025-04-01'), (5, 100, '2025-05-01');
