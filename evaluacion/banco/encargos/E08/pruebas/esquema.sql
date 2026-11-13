CREATE TABLE clientes (id INTEGER PRIMARY KEY, nombre TEXT NOT NULL, pais TEXT NOT NULL);
CREATE TABLE ventas (
    id INTEGER PRIMARY KEY,
    cliente_id INTEGER NOT NULL REFERENCES clientes(id),
    importe INTEGER NOT NULL,
    fecha TEXT NOT NULL
);
