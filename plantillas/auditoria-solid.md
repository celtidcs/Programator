# Referencia para auditorías con vocabulario SOLID

Pega este fichero junto al encargo cuando le pidas a Programator que busque violaciones de
principios de diseño. No cambia lo que el modelo entiende de SOLID, pero fija el error que más se
repite antes de que vuelva a cometerlo.

## El error que más se repite: confundir uso de lo propio con Inversión de Dependencias

**No es una violación de DIP** que un método llame a otro método de su misma clase, use un campo
propio, o llame a un método estático de su propia clase. DIP habla de una clase que depende
directamente de otra clase concreta ajena en vez de depender de una abstracción. Usar lo que ya es
tuyo no es una dependencia hacia fuera: es, sencillamente, cómo funciona una clase.

Correcto (no es DIP): un método de `Inventario` lee `this.listaDeObjetos`, un campo de la propia
clase, o llama a `this.Validar()`, un método propio.

Incorrecto (sí es DIP): un método de `ServicioDePedidos` crea directamente un
`new BaseDeDatosMySql()` y llama a sus métodos, en vez de recibir una abstracción
(`IAlmacenPedidos`) de la que `BaseDeDatosMySql` sea una implementación.

La pregunta que distingue los dos casos: ¿la clase depende de **otra clase concreta** que podría
cambiar por otra implementación, o está usando **lo que ya es suyo**? Si es lo segundo, no hay nada
que señalar.

## Responsabilidad Única (SRP)

Correcto: una clase `Factura` que solo calcula totales e impuestos.

Incorrecto: una clase `Factura` que calcula totales, además de guardarse en la base de datos,
además de formatearse como PDF y enviarse por correo. Tres motivos de cambio distintos en una sola
clase.

## Abierto/Cerrado (OCP)

Correcto: añadir un nuevo tipo de descuento creando una clase nueva que implementa una interfaz
`IDescuento` ya existente, sin tocar el código que ya calculaba otros descuentos.

Incorrecto: un método con un `switch` sobre un tipo de descuento que hay que editar —añadiendo un
`case` más— cada vez que aparece un descuento nuevo.

**Ojo con el falso positivo aquí**: un `switch` o un `if/else` no es, por sí solo, una violación de
OCP. Lo es cuando ese mismo `switch` se repite en varios sitios del código y hay que tocarlos todos
a la vez para añadir un caso. Uno solo, en un único sitio, suele ser la solución más simple y no
hace falta señalarlo.

## Sustitución de Liskov (LSP)

Correcto: una subclase `Pinguino` de `Ave` que sobrescribe `Moverse()` para nadar en vez de volar,
sin que el código que usa `Ave` tenga que saber si es un pingüino o no.

Incorrecto: una subclase `Pinguino` de `Ave` donde `Volar()` lanza una excepción, porque el código
que trata cualquier `Ave` como capaz de volar se rompe en cuanto recibe un pingüino.

## Segregación de Interfaces (ISP)

Correcto: una interfaz `ILector` con un único método `Leer()`, separada de `IEscritor` con
`Escribir()`, para que una clase de solo lectura no tenga que implementar `Escribir()` sin sentido.

Incorrecto: una interfaz `IRepositorio` con diez métodos, donde la mitad de las clases que la
implementan dejan la otra mitad de los métodos vacíos o lanzando «no implementado».

**Esto solo aplica a una clase que depende de una interfaz excesiva, no a cualquier interfaz
grande.** Una interfaz con muchos métodos que todas sus implementaciones usan de verdad no es un
problema de ISP.
