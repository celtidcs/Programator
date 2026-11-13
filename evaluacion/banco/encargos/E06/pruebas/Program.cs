// Pruebas que el modelo no ve, sobre su LectorLotes.cs.
// Uso: dotnet salida/banco_e06.dll <nombre>. Termina con 0 si pasa, 1 si falla y 2 si el nombre no existe.

using System.Text;

var pruebas = new Dictionary<string, Func<Task<bool>>>
{
    ["lotea_siete_lineas_de_tres_en_tres"] = async () =>
    {
        var tamanos = new List<int>();
        await using var lector = new LectorLotes(Fichero(7));
        await foreach (var lote in lector.LeerLotesAsync(3)) tamanos.Add(lote.Count);
        return tamanos.SequenceEqual(new[] { 3, 3, 1 });
    },
    // Si el lector reutiliza la misma lista y la vacía para el lote siguiente, lo ya entregado cambia.
    ["los_lotes_entregados_no_se_vacian"] = async () =>
    {
        var guardados = new List<IReadOnlyList<string>>();
        await using var lector = new LectorLotes(Fichero(6));
        await foreach (var lote in lector.LeerLotesAsync(3)) guardados.Add(lote);
        return guardados.Count == 2
            && guardados[0].SequenceEqual(new[] { "linea 1", "linea 2", "linea 3" })
            && guardados[1].SequenceEqual(new[] { "linea 4", "linea 5", "linea 6" });
    },
    ["tamano_cero_lanza_argument_out_of_range"] = () => LanzaFueraDeRango(0),
    ["tamano_negativo_lanza_argument_out_of_range"] = () => LanzaFueraDeRango(-3),
    ["respeta_la_cancelacion"] = async () =>
    {
        await using var lector = new LectorLotes(Fichero(100));
        using var cancelacion = new CancellationTokenSource();
        var lotes = 0;
        try
        {
            await foreach (var lote in lector.LeerLotesAsync(2, cancelacion.Token))
            {
                lotes++;
                cancelacion.Cancel();
            }
        }
        catch (OperationCanceledException)
        {
            return lotes == 1;
        }
        return false;
    },
    ["un_fichero_vacio_no_emite_lotes"] = async () =>
    {
        var lotes = 0;
        await using var lector = new LectorLotes(Fichero(0));
        await foreach (var lote in lector.LeerLotesAsync(4)) lotes++;
        return lotes == 0;
    },
    ["un_lote_mayor_que_el_fichero_lo_devuelve_entero"] = async () =>
    {
        var tamanos = new List<int>();
        await using var lector = new LectorLotes(Fichero(5));
        await foreach (var lote in lector.LeerLotesAsync(100)) tamanos.Add(lote.Count);
        return tamanos.SequenceEqual(new[] { 5 });
    },
};

var nombre = args.Length > 0 ? args[0] : "";
if (!pruebas.TryGetValue(nombre, out var prueba))
{
    Console.Error.WriteLine($"no existe la prueba «{nombre}»");
    return 2;
}
bool pasa;
try
{
    pasa = await prueba();
}
catch (Exception fallo)
{
    Console.Error.WriteLine($"lanzó {fallo.GetType().Name}: {fallo.Message}");
    pasa = false;
}
Console.WriteLine($"{(pasa ? "PASA " : "FALLA")}  {nombre}");
return pasa ? 0 : 1;

static string Fichero(int lineas)
{
    var ruta = Path.Combine(Path.GetTempPath(), $"banco-e06-{Guid.NewGuid():N}.txt");
    var texto = new StringBuilder();
    for (var i = 1; i <= lineas; i++) texto.Append($"linea {i}\n");
    File.WriteAllText(ruta, texto.ToString());
    return ruta;
}

// Se acepta que lance al llamar o al empezar a enumerar: el enunciado no exige cuál.
static async Task<bool> LanzaFueraDeRango(int tamano)
{
    await using var lector = new LectorLotes(Fichero(3));
    try
    {
        await foreach (var lote in lector.LeerLotesAsync(tamano)) { }
    }
    catch (ArgumentOutOfRangeException)
    {
        return true;
    }
    return false;
}
