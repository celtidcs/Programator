# Defectos conocidos y limitaciones estructurales

Este documento recoge de forma explícita los defectos conocidos, las pruebas no automatizadas y las limitaciones de diseño que siguen vigentes en Programator. Su propósito es advertir con claridad de los puntos débiles del sistema para que quien lo utilice sepa qué esperar, qué consecuencias operativas existen y cómo mitigar cada situación.

## Pruebas ignoradas en la suite automatizada

La suite mecánica del proyecto contiene dos pruebas unitarias marcadas con el atributo `#[ignore]`. Estas pruebas no se ejecutan durante la verificación ordinaria con `cargo test` porque requieren acceso directo al hardware físico de la máquina:

1. Detección de tarjeta gráfica en `src/motor/hardware.rs`: La prueba `imprime_lo_que_ve_describir_gpu_en_esta_maquina` consulta la interfaz NVML del sistema para obtener el modelo de GPU y la memoria VRAM disponible. Al depender de la presencia física de una tarjeta compatible, no puede ejecutarse en entornos de integración continua desprovistos de aceleración por hardware.
2. Comprobación del modelo real en `src/motor/gguf/mod.rs`: La prueba `el_peso_del_devstral_real_cuadra_con_el_tamano_del_fichero` valida la coherencia de los metadatos y tensores frente al archivo GGUF de 13,3 GiB. Requiere que el modelo completo esté descargado y sea accesible en el disco.

Consecuencia para el usuario: La junta de comunicación entre el arnés y el motor de inferencia real queda fuera de la verificación automática en frío. Una modificación inadvertida en los parámetros de arranque del motor podría compilar limpiamente y superar todas las pruebas automáticas, pero fallar en el momento de encender la tarjeta gráfica.

Cómo mitigarlo: Antes de dar por buena una entrega para producción, es obligatorio ejecutar manualmente las pruebas ignoradas y seguir el protocolo de comprobación física descrito en `docs/pruebas-manuales.md`.

## Limitaciones estructurales pendientes de evolución

Durante la jornada de trabajo real en NatureLand (23 de septiembre de 2026), se identificaron cuatro limitaciones del protocolo de comunicación que actualmente permanecen pospuestas para no alterar de forma abrupta el contrato con los agentes coordinadores:

### 1. Detección de encargos por marcas en archivo continuo en lugar de archivos individuales

Actualmente, los encargos se redactan como secciones bajo encabezados específicos dentro del archivo de buzón de cada agente. El arnés rastrea los avances de lectura midiendo el desplazamiento en bytes dentro del archivo.

Consecuencia para el usuario: Si un usuario o agente edita el contenido anterior de un buzón en lugar de añadir una sección al final, o si la herramienta de edición altera los saltos de línea del archivo, los punteros de lectura pueden desorientarse o pasar por alto el encargo.

Cómo mitigarlo: Redactar siempre cada nuevo encargo como una sección agregada estrictamente al final del buzón, encabezada por `## Para Programator`, sin modificar el historial previo.

### 2. Ausencia de cola formal de encargos y cancelación explícita

El arnés procesa los encargos secuencialmente en el orden en que los detecta durante el sondeo del canal. No existe un mecanismo formal para definir prioridades, cancelar un encargo que ya ha entrado en procesamiento o reorganizar la lista de tareas pendientes.

Consecuencia para el usuario: Si se publica un encargo erróneo, el modelo consumirá tiempo de inferencia en la tarjeta gráfica hasta completar el ciclo o agotar el cupo de herramientas, demorando la atención de encargos posteriores.

Cómo mitigarlo: Verificar cuidadosamente los requisitos y el alcance de cada encargo antes de publicarlo en el buzón.

### 3. Falta de catálogo persistente de entregas previas

Programator inicia cada ciclo de inferencia sin memoria de las sesiones anteriores. El arnés no mantiene un índice estructurado de las propuestas aprobadas o entregadas en días previos, por lo que el modelo no conoce las funciones o tipos que él mismo generó en encargos anteriores a menos que se le indique leer los archivos correspondientes.

Consecuencia para el usuario: El modelo puede sugerir nombres duplicados o reinventar estructuras ya entregadas si el encargo no le instruye explícitamente a inspeccionar el código preexistente mediante la herramienta `leer_fichero`.

Cómo mitigarlo: Incluir en las instrucciones del encargo la indicación de comprobar los archivos relevantes del proyecto antes de escribir código nuevo.

### 4. Seguimiento de estado por propuesta ausente

Las propuestas generadas por el modelo se depositan en el directorio `.gestor/candidatos/programator/` como archivos aislados. El sistema no dispone de un registro que clasifique automáticamente si una propuesta fue finalmente aceptada, rechazada o sustituida por una versión posterior.

Consecuencia para el usuario: El directorio de candidatos acumula propuestas a lo largo de las sesiones, obligando al equipo humano o al agente coordinador a realizar un seguimiento manual de cuáles han sido integradas en el proyecto.

Cómo mitigarlo: Limpiar periódicamente el directorio de candidatos o archivar las propuestas que hayan sido definitivamente descartadas tras la verificación.

## Incidencia sobre la primera pasada del canal

Conviene dejar constancia de que la denominada «incidencia C3» del informe de NatureLand ha quedado descartada como defecto. La guía de usuario sugería erróneamente que en cada inicio del programa se ignoraban los textos preexistentes en el canal.

La inspección del código confirmó que dicha instantánea inicial se ejecuta únicamente la primera vez que Programator se conecta a un proyecto nuevo, cuando el archivo de registro aún no existe. En cualquier arranque sucesivo, Programator conserva su memoria de lectura y atiende cualquier encargo publicado con posterioridad a la última revisión, garantizando un comportamiento predecible.
