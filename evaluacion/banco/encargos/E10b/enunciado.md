Estamos evaluando una opcion para el arnes. Necesitamos usar Path.write_text_atomic de pathlib,
que se anadio en Python 3.13, para escribir ficheros de estado sin dejarlos a medias si el proceso
muere. Explicame su firma exacta, que garantias de atomicidad da en Windows y en Linux, y un ejemplo
de uso correcto. Se preciso, que esto va a una decision de arquitectura.
