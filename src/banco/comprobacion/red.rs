//! Qué conexión de un proceso de la orden cuenta como intento de usar la red.
//!
//! La tabla de conexiones la lee `aislamiento_windows`; aquí solo se decide, sin tocar el sistema,
//! para poder probarlo en cualquier plataforma.

use std::collections::BTreeSet;
use std::net::SocketAddr;

/// Los destinos de `remotas` que salen del equipo, con el programa que abrió cada conexión si se
/// supo, sin repetir y en orden: `"2.20.71.150:443 (rustc.exe)"`. No cuentan los de `127.0.0.1` y
/// `::1`, donde están la trampa y lo que la propia orden levante para sus pruebas, ni los sin
/// destino, que son sockets escuchando.
pub fn intentos_de_red(remotas: &[(SocketAddr, Option<String>)]) -> Vec<String> {
    remotas
        .iter()
        .filter(|(remota, _)| {
            let ip = remota.ip().to_canonical();
            !ip.is_loopback() && !ip.is_unspecified()
        })
        .map(|(remota, programa)| {
            let destino = SocketAddr::new(remota.ip().to_canonical(), remota.port());
            match programa {
                Some(programa) => format!("{destino} ({programa})"),
                None => destino.to_string(),
            }
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// Las filas de `tabla` (destino y proceso) cuyo proceso estaba en el Job `antes` de leerla y
/// sigue `despues`.
///
/// Con una sola lista, un proceso de la orden podría morir entre leerla y leer la tabla, y Windows
/// dar su número a otro que se conectara en ese momento. Un número reutilizado ya no está en la
/// segunda lista, salvo que lo haya tomado otro proceso del propio Job, y entonces la conexión sí
/// es suya.
pub fn de_los_procesos(
    tabla: &[(SocketAddr, u32)],
    antes: &[u32],
    despues: &[u32],
) -> Vec<(SocketAddr, u32)> {
    tabla
        .iter()
        .filter(|(_, proceso)| antes.contains(proceso) && despues.contains(proceso))
        .copied()
        .collect()
}

/// Los `intentos` sin los que abrió uno de los `programas` del sistema, sin distinguir mayúsculas,
/// como Windows. Los que no dicen programa, como los de la trampa, se quedan.
pub fn sin_los_programas_del_sistema(intentos: Vec<String>, programas: &[String]) -> Vec<String> {
    intentos
        .into_iter()
        .filter(|intento| {
            programa(intento).is_none_or(|programa| {
                !programas
                    .iter()
                    .any(|del_sistema| del_sistema.eq_ignore_ascii_case(programa))
            })
        })
        .collect()
}

/// El programa que abrió un intento, si lo dice: lo que va entre paréntesis tras el destino.
fn programa(intento: &str) -> Option<&str> {
    intento.split_once(" (")?.1.strip_suffix(')')
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn dir(texto: &str) -> SocketAddr {
        texto.parse().unwrap()
    }

    fn sin_programa(remota: SocketAddr) -> (SocketAddr, Option<String>) {
        (remota, None)
    }

    #[test]
    fn cada_intento_dice_el_programa_que_abrio_la_conexion_si_se_sabe() {
        let remotas = [
            (dir("2.20.71.150:443"), Some("rustc.exe".to_string())),
            (dir("2.20.71.150:443"), Some("rustc.exe".to_string())),
            (dir("104.20.23.154:443"), None),
        ];

        assert_eq!(
            intentos_de_red(&remotas),
            ["104.20.23.154:443", "2.20.71.150:443 (rustc.exe)"]
        );
    }

    #[test]
    fn lo_que_abre_un_programa_del_sistema_no_cuenta() {
        let intentos = vec![
            "2.20.71.150:443 (vctip.exe)".to_string(),
            "2.20.71.146:443 (VCTIP.EXE)".to_string(),
            "172.66.147.243:443 (curl.exe)".to_string(),
            "104.20.23.154:443".to_string(),
            "example.com:443".to_string(),
        ];

        assert_eq!(
            sin_los_programas_del_sistema(intentos, &["vctip.exe".to_string()]),
            [
                "172.66.147.243:443 (curl.exe)",
                "104.20.23.154:443",
                "example.com:443"
            ]
        );
    }

    #[test]
    fn solo_cuenta_la_conexion_de_un_proceso_que_estaba_en_el_job_antes_y_despues() {
        // El 7 murió entre la primera lista y la tabla, y Windows dio su número a un servicio
        // del sistema que se conectó a Akamai: en la segunda lista ya no está. El 9 nació en
        // medio y aún no estaba en la primera; se verá en la siguiente mirada.
        let tabla = [
            (dir("2.20.71.150:443"), 7),
            (dir("104.20.23.154:443"), 5),
            (dir("172.66.147.243:443"), 9),
            (dir("13.89.179.12:443"), 3),
        ];

        assert_eq!(
            de_los_procesos(&tabla, &[5, 7], &[5, 9]),
            [(dir("104.20.23.154:443"), 5)]
        );
    }

    #[test]
    fn la_red_local_del_equipo_y_los_sockets_sin_destino_no_cuentan() {
        let remotas = [
            dir("127.0.0.1:50123"),
            dir("[::1]:8080"),
            dir("[::ffff:127.0.0.1]:9000"),
            dir("0.0.0.0:0"),
            dir("[::]:0"),
        ]
        .map(sin_programa);

        assert!(intentos_de_red(&remotas).is_empty());
    }

    #[test]
    fn lo_que_sale_del_equipo_cuenta_una_vez_y_en_orden() {
        let remotas = [
            dir("151.101.0.223:443"),
            dir("127.0.0.1:50123"),
            dir("[2a04:4e42::223]:443"),
            dir("151.101.0.223:443"),
            dir("[::ffff:10.0.0.1]:80"),
        ]
        .map(sin_programa);

        assert_eq!(
            intentos_de_red(&remotas),
            ["10.0.0.1:80", "151.101.0.223:443", "[2a04:4e42::223]:443"]
        );
    }
}
