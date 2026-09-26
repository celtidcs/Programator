//! Incrusta el icono y los datos de versión en el ejecutable de Windows.
//!
//! Solo actúa en Windows y solo si el icono está. Si falta el recurso o no hay con qué compilarlo,
//! avisa y sigue: quedarse sin icono es un detalle, no poder compilar el programa no lo es.

fn main() {
    println!("cargo:rerun-if-changed=recursos/programator.ico");

    #[cfg(windows)]
    {
        if std::path::Path::new("recursos/programator.ico").exists() {
            let mut recursos = winresource::WindowsResource::new();
            recursos.set_icon("recursos/programator.ico");
            recursos.set("ProductName", "Programator");
            recursos.set(
                "FileDescription",
                "Arnés que convierte un modelo local en un agente del canal",
            );
            recursos.set("LegalCopyright", "Proyecto Programator");
            if let Err(fallo) = recursos.compile() {
                println!("cargo:warning=no se pudo incrustar el icono: {fallo}");
            }
        }
    }
}
