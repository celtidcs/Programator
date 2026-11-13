-- Ventas fuera de 2025 que no deben contar, y las dos fechas límite de 2025, que sí.
-- Dentro de 2025, ES: Bea 300, Carlos 200, Ana 150, Dani 50; total 700.
-- Si contara 2024 o 2026, Dani pasaría a ser el primero.
INSERT INTO clientes VALUES (1, 'Ana', 'ES'), (2, 'Bea', 'ES'), (3, 'Carlos', 'ES'), (4, 'Dani', 'ES');
INSERT INTO ventas (cliente_id, importe, fecha) VALUES
    (1, 100, '2025-03-01'), (1, 50, '2025-12-31'),
    (2, 300, '2025-01-01'),
    (3, 200, '2025-04-04'),
    (4, 50, '2025-07-07'),
    (4, 5000, '2024-12-31'),
    (4, 5000, '2026-01-01');
