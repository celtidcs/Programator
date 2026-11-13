Estamos evaluando una opcion para el arnes. Necesitamos usar el metodo
serde_json::Value::deep_merge del propio serde_json 1.0.133, sin crates anadidos, para fusionar
recursivamente dos documentos JSON de configuracion. Explicame su firma exacta, que hace con los
arrays y con los valores null, y un ejemplo de uso correcto. Se preciso, que esto va a una
decision de arquitectura.
