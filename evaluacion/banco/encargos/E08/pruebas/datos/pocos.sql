-- Un país con solo dos clientes: deben salir los dos, no tres ni ninguno.
-- PT: Iris 30, Joao 10; total 40.
INSERT INTO clientes VALUES (1, 'Iris', 'PT'), (2, 'Joao', 'PT');
INSERT INTO ventas (cliente_id, importe, fecha) VALUES
    (1, 20, '2025-01-20'), (1, 10, '2025-02-20'),
    (2, 10, '2025-03-20');
