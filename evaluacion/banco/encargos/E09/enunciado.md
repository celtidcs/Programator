Este generador de Python pasa las pruebas de su autor pero corrompe datos en produccion.
Encuentra el fallo, explica por que ocurre y di como se arregla. No hace falta que entregues
fichero: publica la respuesta en tu buzon.

    def lotes(items, n):
        lote = []
        for x in items:
            lote.append(x)
            if len(lote) == n:
                yield lote
                lote.clear()
        if lote:
            yield lote

La prueba de su autor era esta, y pasa:

    assert [len(l) for l in lotes(range(7), 3)] == [3, 3, 1]

Pero este otro uso falla:

    guardados = list(lotes(range(6), 3))
    print(guardados)   # imprime [[], []]
