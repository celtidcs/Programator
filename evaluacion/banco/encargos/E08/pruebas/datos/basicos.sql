-- Dos países de cuatro clientes, todo en 2025 y sin empates.
-- ES: Bea 300, Carlos 200, Ana 150, Dani 50; total 700.
-- FR: Eva 1000, Fabien 500, Gil 250, Hugo 100; total 1850.
INSERT INTO clientes VALUES
    (1, 'Ana', 'ES'), (2, 'Bea', 'ES'), (3, 'Carlos', 'ES'), (4, 'Dani', 'ES'),
    (5, 'Eva', 'FR'), (6, 'Fabien', 'FR'), (7, 'Gil', 'FR'), (8, 'Hugo', 'FR');
INSERT INTO ventas (cliente_id, importe, fecha) VALUES
    (1, 100, '2025-03-01'), (1, 50, '2025-06-15'),
    (2, 300, '2025-02-10'),
    (3, 120, '2025-04-04'), (3, 80, '2025-09-30'),
    (4, 50, '2025-07-07'),
    (5, 1000, '2025-05-05'),
    (6, 500, '2025-08-08'),
    (7, 250, '2025-10-10'),
    (8, 100, '2025-11-11');
