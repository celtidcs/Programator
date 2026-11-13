using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Text;

namespace Tienda;

public sealed record Linea(string Producto, int Unidades, decimal PrecioUnitario);

public sealed record Pedido(int Id, string Cliente, string Pais, IReadOnlyList<Linea> Lineas, bool EsSocio);

public static class Tarifas
{
    public const decimal DescuentoSocio = 0.05m;
    public const decimal RecargoUrgente = 4.90m;
    public const int MaximoUnidadesPorLinea = 99;
    public const int LongitudCodigoPais = 2;
}

public sealed class ServicioPedidos
{
    private readonly List<Pedido> _pedidos = new();

    public void Registrar(Pedido pedido)
    {
        ArgumentNullException.ThrowIfNull(pedido);
        _pedidos.Add(pedido);
    }

    public decimal Subtotal(Pedido pedido)
    {
        return pedido.Lineas.Sum(l => l.Unidades * l.PrecioUnitario);
    }

    public decimal TotalConImpuestos(Pedido pedido)
    {
        var subtotal = Subtotal(pedido);
        if (pedido.EsSocio)
        {
            subtotal -= subtotal * Tarifas.DescuentoSocio;
            subtotal = Math.Round(subtotal, 2, MidpointRounding.AwayFromZero);
        }
        return subtotal + subtotal * 0.21m;
    }

    public decimal GastosDeEnvio(Pedido pedido)
    {
        var subtotal = Subtotal(pedido);
        if (subtotal >= 50m)
        {
            return 0m;
        }
        return pedido.Pais == "ES" ? 3.95m : 9.95m;
    }

    public decimal TotalUrgente(Pedido pedido)
    {
        var subtotal = Subtotal(pedido);
        if (pedido.EsSocio)
        {
            subtotal -= subtotal * Tarifas.DescuentoSocio;
            subtotal = Math.Round(subtotal, 2, MidpointRounding.AwayFromZero);
        }
        return subtotal + Tarifas.RecargoUrgente;
    }

    public IReadOnlyList<string> Validar(Pedido pedido)
    {
        var errores = new List<string>();
        if (string.IsNullOrWhiteSpace(pedido.Cliente))
        {
            errores.Add("el pedido no tiene cliente");
        }
        if (pedido.Lineas.Count == 0)
        {
            errores.Add("el pedido no tiene líneas");
        }
        foreach (var linea in pedido.Lineas)
        {
            if (string.IsNullOrWhiteSpace(linea.Producto))
            {
                errores.Add("hay una línea sin producto");
            }
            if (linea.Unidades <= 0)
            {
                errores.Add($"{linea.Producto}: las unidades deben ser positivas");
            }
            if (linea.Unidades > Tarifas.MaximoUnidadesPorLinea)
            {
                errores.Add($"{linea.Producto}: demasiadas unidades en una línea");
            }
            if (linea.PrecioUnitario < 0)
            {
                errores.Add($"{linea.Producto}: el precio no puede ser negativo");
            }
        }
        var repetidos = pedido.Lineas.GroupBy(l => l.Producto).Where(g => g.Count() > 1).Select(g => g.Key);
        foreach (var producto in repetidos)
        {
            errores.Add($"{producto}: aparece en varias líneas");
        }
        if (pedido.Pais.Length != Tarifas.LongitudCodigoPais)
        {
            errores.Add("el país debe ser un código de dos letras");
        }
        if (pedido.Id <= 0)
        {
            errores.Add("el identificador del pedido debe ser positivo");
        }
        var total = Subtotal(pedido);
        if (total == 0 && pedido.Lineas.Count > 0)
        {
            errores.Add("un pedido con líneas no puede salir gratis");
        }
        if (pedido.Lineas.Sum(l => l.Unidades) > Tarifas.MaximoUnidadesPorLinea * pedido.Lineas.Count)
        {
            errores.Add("el pedido supera el máximo de unidades");
        }
        return errores;
    }

    public void GuardarInformeCsv(string ruta)
    {
        var texto = new StringBuilder();
        texto.AppendLine("id;cliente;pais;subtotal");
        foreach (var pedido in _pedidos)
        {
            texto.AppendLine($"{pedido.Id};{pedido.Cliente};{pedido.Pais};{Subtotal(pedido)}");
        }
        File.WriteAllText(ruta, texto.ToString(), Encoding.UTF8);
    }

    public IReadOnlyList<Pedido> DeCliente(string cliente)
    {
        return _pedidos.Where(p => p.Cliente == cliente).ToList();
    }
}
