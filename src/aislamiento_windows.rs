//! Las llamadas a Windows que aíslan la comprobación local: la integridad baja y el Job Object.
//!
//! Es el único fichero del proyecto con `unsafe`. Cada bloque dice qué garantiza el código que lo
//! rodea para que la llamada sea correcta. Lo que decide cuándo y con qué se llama vive fuera, en
//! código seguro.
//!
//! - **Integridad baja.** Un proceso de integridad baja no puede escribir en lo que tiene
//!   integridad media, que es casi todo lo del usuario. El ayudante baja la de su propio testigo,
//!   algo que Windows deja hacer sin privilegios, y lo que lance la hereda.
//! - **La carpeta marcada.** `comprobacion/` lleva una etiqueta de integridad baja heredable, para
//!   que el código del modelo pueda trabajar dentro de ella y en ningún otro sitio.
//! - **El Job.** Limita procesos y memoria, y al cerrarse mata todo el árbol.
//! - **La red.** Qué procesos hay en el Job y qué conexiones TCP tienen abiertas, para el
//!   detector de red. Decidir cuáles cuentan es de `banco::comprobacion::red`.

use std::ffi::{c_void, OsStr};
use std::iter::once;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::AsRawHandle;
use std::path::Path;
use std::process::Child;
use std::ptr;

use windows_sys::Win32::Foundation::WIN32_ERROR;
use windows_sys::Win32::Foundation::{
    CloseHandle, LocalFree, ERROR_INSUFFICIENT_BUFFER, ERROR_MORE_DATA, ERROR_SUCCESS, HANDLE,
};
use windows_sys::Win32::NetworkManagement::IpHelper::{
    GetExtendedTcpTable, MIB_TCP6ROW_OWNER_PID, MIB_TCPROW_OWNER_PID, TCP_TABLE_OWNER_PID_ALL,
};
use windows_sys::Win32::Networking::WinSock::{AF_INET, AF_INET6};
use windows_sys::Win32::Security::Authorization::{
    ConvertStringSecurityDescriptorToSecurityDescriptorW, ConvertStringSidToSidW,
    GetNamedSecurityInfoW, SetEntriesInAclW, SetNamedSecurityInfoW, EXPLICIT_ACCESS_W,
    GRANT_ACCESS, NO_MULTIPLE_TRUSTEE, SDDL_REVISION_1, SE_FILE_OBJECT, TRUSTEE_IS_SID,
    TRUSTEE_IS_USER, TRUSTEE_W,
};
use windows_sys::Win32::Security::{
    GetLengthSid, GetSecurityDescriptorSacl, GetTokenInformation, SetTokenInformation,
    TokenIntegrityLevel, TokenUser, ACL, DACL_SECURITY_INFORMATION, LABEL_SECURITY_INFORMATION,
    NO_INHERITANCE, SID_AND_ATTRIBUTES, TOKEN_ADJUST_DEFAULT, TOKEN_MANDATORY_LABEL, TOKEN_QUERY,
    TOKEN_USER,
};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JobObjectBasicProcessIdList,
    JobObjectExtendedLimitInformation, QueryInformationJobObject, SetInformationJobObject,
    TerminateJobObject, JOBOBJECT_BASIC_PROCESS_ID_LIST, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JOB_OBJECT_LIMIT_ACTIVE_PROCESS, JOB_OBJECT_LIMIT_JOB_MEMORY,
    JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};
use windows_sys::Win32::System::SystemServices::SE_GROUP_INTEGRITY;
use windows_sys::Win32::System::Threading::{
    GetCurrentProcess, OpenProcess, OpenProcessToken, QueryFullProcessImageNameW,
    PROCESS_QUERY_LIMITED_INFORMATION,
};

use crate::error::{Error, Resultado};

/// El SID del nivel de integridad bajo, `SECURITY_MANDATORY_LOW_RID`.
const SID_DE_INTEGRIDAD_BAJA: &str = "S-1-16-4096";
/// La etiqueta de la carpeta, en SDDL: obligatoria (`ML`), heredada por ficheros y carpetas
/// (`OICI`), que impide escribir a quien esté por debajo (`NW`) del nivel bajo (`LW`). Con ella,
/// un proceso de integridad baja puede escribir dentro.
const ETIQUETA_DE_INTEGRIDAD_BAJA: &str = "S:(ML;OICI;NW;;;LW)";
/// Bytes en un MiB, para pasar `memoria_maxima_mib` al Job.
const BYTES_POR_MIB: usize = 1024 * 1024;
/// El código de salida con que terminan los procesos del Job al cortarlo. Cualquiera distinto de
/// cero: lo que importa es que no parezca un éxito.
const CODIGO_AL_CORTAR: u32 = 1;
/// Cuántas veces se intenta leer la tabla de conexiones. Entre preguntar su tamaño y leerla pueden
/// abrirse otras; tres veces seguidas creciendo ya no es mala suerte, y se deja para la siguiente
/// vuelta.
const INTENTOS_DE_LEER_LA_TABLA: usize = 3;
/// Caracteres del búfer para la ruta de un ejecutable: el máximo de una ruta larga de Windows.
const LARGO_MAXIMO_DE_RUTA: usize = 32_767;
/// Bytes de cada palabra del búfer de la tabla: las filas son de `u32` y bytes sueltos.
const BYTES_POR_PALABRA: usize = std::mem::size_of::<u32>();
/// El permiso de cambiar el propietario de un objeto, que es el que pide Windows para ponerle una
/// etiqueta obligatoria: `WRITE_OWNER` de `winnt.h`. Se nombra aquí para no traer la característica
/// `Win32_Storage_FileSystem` de `windows-sys` solo por esta constante.
const PERMISO_DE_CAMBIAR_EL_PROPIETARIO: u32 = 0x0008_0000;
/// Bytes de cada palabra del búfer de `TOKEN_USER`: de `u64` para que quede alineado como un
/// puntero, que es su primer campo.
const BYTES_POR_PALABRA_DEL_USUARIO: usize = std::mem::size_of::<u64>();

/// El fallo de la última llamada a Windows, con su nombre.
fn fallo_de(llamada: &str) -> Error {
    Error::Aislamiento(format!("{llamada}: {}", std::io::Error::last_os_error()))
}

/// Un texto en UTF-16 terminado en cero, como lo piden las funciones `…W`.
fn ancho(texto: &OsStr) -> Vec<u16> {
    texto.encode_wide().chain(once(0)).collect()
}

/// Un asa de Windows que se cierra al soltarla.
struct Asa(HANDLE);

impl Drop for Asa {
    fn drop(&mut self) {
        // SAFETY: `Asa` solo se construye con un asa válida recién devuelta por Windows, y nadie
        // más la cierra.
        unsafe { CloseHandle(self.0) };
    }
}

/// Memoria que Windows reservó con `LocalAlloc` y que hay que devolver con `LocalFree`.
struct Local(*mut c_void);

impl Drop for Local {
    fn drop(&mut self) {
        // SAFETY: el puntero lo devolvió una función de Windows que pide liberarlo con
        // `LocalFree`, y solo se libera aquí.
        unsafe { LocalFree(self.0) };
    }
}

/// El código que devuelven las funciones de seguridad sobre una carpeta, como resultado.
fn sobre_la_carpeta(codigo: WIN32_ERROR, llamada: &str, carpeta: &Path) -> Resultado<()> {
    if codigo == ERROR_SUCCESS {
        return Ok(());
    }
    Err(Error::Aislamiento(format!(
        "{llamada} sobre «{}»: {}",
        carpeta.display(),
        std::io::Error::from_raw_os_error(codigo as i32)
    )))
}

/// El testigo de este proceso, abierto con `acceso`.
fn abrir_mi_testigo(acceso: u32) -> Resultado<Asa> {
    let mut testigo: HANDLE = ptr::null_mut();
    // SAFETY: `GetCurrentProcess` da una pseudoasa que siempre vale; `testigo` es una variable
    // local donde Windows deja el asa, que `Asa` cierra.
    if unsafe { OpenProcessToken(GetCurrentProcess(), acceso, &mut testigo) } == 0 {
        return Err(fallo_de("OpenProcessToken"));
    }
    Ok(Asa(testigo))
}

/// El usuario de este proceso: un `TOKEN_USER` en un búfer alineado, del que sale su SID.
struct MiUsuario(Vec<u64>);

impl MiUsuario {
    fn leer() -> Resultado<Self> {
        let testigo = abrir_mi_testigo(TOKEN_QUERY)?;
        let mut largo = 0;
        // SAFETY: sin búfer, la llamada solo dice en `largo` cuánto ocupa; que falle es lo
        // esperado.
        unsafe { GetTokenInformation(testigo.0, TokenUser, ptr::null_mut(), 0, &mut largo) };
        if largo == 0 {
            return Err(fallo_de("GetTokenInformation"));
        }
        let mut bufer = vec![0u64; (largo as usize).div_ceil(BYTES_POR_PALABRA_DEL_USUARIO)];
        // SAFETY: `bufer` tiene al menos `largo` bytes, vive durante la llamada y está alineado
        // a ocho, lo que pide `TOKEN_USER`.
        let leido = unsafe {
            GetTokenInformation(
                testigo.0,
                TokenUser,
                bufer.as_mut_ptr().cast(),
                largo,
                &mut largo,
            )
        };
        if leido == 0 {
            return Err(fallo_de("GetTokenInformation"));
        }
        Ok(Self(bufer))
    }

    /// Su SID. Apunta dentro del búfer: vale mientras viva `self`.
    fn sid(&self) -> *mut c_void {
        // SAFETY: `GetTokenInformation` llenó el búfer con un `TOKEN_USER`, y está alineado.
        unsafe { (*self.0.as_ptr().cast::<TOKEN_USER>()).User.Sid }
    }
}

/// Da al usuario de este proceso el permiso de cambiar el propietario de `carpeta`, que es el
/// que pide Windows para etiquetarla.
///
/// Hace falta donde el usuario solo tiene «Modificar»: en una unidad que lo hereda de
/// «Usuarios autentificados», la etiqueta fallaba con «Acceso denegado» (visto en uso
/// real el 13/11/2026). Quien crea la carpeta es su propietario, y el propietario siempre puede cambiar la
/// DACL. Solo para esta carpeta, sin herencia: lo que se cree dentro hereda la etiqueta, no esto.
fn darme_permiso_de_etiquetar(carpeta: &Path, ruta: &[u16]) -> Resultado<()> {
    let mut dacl: *mut ACL = ptr::null_mut();
    let mut descriptor = ptr::null_mut();
    // SAFETY: `ruta` termina en cero y vive durante la llamada; `dacl` y `descriptor` son
    // variables locales. `dacl` apunta dentro del descriptor, que se libera con `LocalFree`.
    let codigo = unsafe {
        GetNamedSecurityInfoW(
            ruta.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            ptr::null_mut(),
            ptr::null_mut(),
            &mut dacl,
            ptr::null_mut(),
            &mut descriptor,
        )
    };
    sobre_la_carpeta(codigo, "GetNamedSecurityInfoW", carpeta)?;
    let _descriptor = Local(descriptor);
    if dacl.is_null() {
        // Sin DACL todo el mundo puede todo; una con una sola entrada lo restringiría.
        return Ok(());
    }

    let usuario = MiUsuario::leer()?;
    let acceso = EXPLICIT_ACCESS_W {
        grfAccessPermissions: PERMISO_DE_CAMBIAR_EL_PROPIETARIO,
        grfAccessMode: GRANT_ACCESS,
        grfInheritance: NO_INHERITANCE,
        Trustee: TRUSTEE_W {
            pMultipleTrustee: ptr::null_mut(),
            MultipleTrusteeOperation: NO_MULTIPLE_TRUSTEE,
            TrusteeForm: TRUSTEE_IS_SID,
            TrusteeType: TRUSTEE_IS_USER,
            ptstrName: usuario.sid().cast(),
        },
    };
    let mut nueva: *mut ACL = ptr::null_mut();
    // SAFETY: `acceso` y el SID al que apunta viven durante la llamada; `dacl` sigue dentro del
    // descriptor vivo; `nueva` es una variable local que se libera con `LocalFree`.
    let codigo = unsafe { SetEntriesInAclW(1, &acceso, dacl, &mut nueva) };
    sobre_la_carpeta(codigo, "SetEntriesInAclW", carpeta)?;
    let nueva = Local(nueva.cast());

    // SAFETY: `ruta` termina en cero y `nueva` es la ACL válida que acaba de crear Windows; las
    // dos viven durante la llamada. Solo se cambia la DACL.
    let codigo = unsafe {
        SetNamedSecurityInfoW(
            ruta.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            ptr::null_mut(),
            ptr::null_mut(),
            nueva.0.cast(),
            ptr::null(),
        )
    };
    sobre_la_carpeta(codigo, "SetNamedSecurityInfoW (permiso)", carpeta)
}

/// Baja la integridad de este proceso al nivel bajo. Todo lo que lance después la hereda.
pub fn bajar_mi_integridad() -> Resultado<()> {
    let texto = ancho(OsStr::new(SID_DE_INTEGRIDAD_BAJA));
    let mut sid = ptr::null_mut();
    // SAFETY: `texto` termina en cero y vive durante la llamada; `sid` es una variable local
    // donde Windows deja un puntero que luego se libera con `LocalFree`.
    if unsafe { ConvertStringSidToSidW(texto.as_ptr(), &mut sid) } == 0 {
        return Err(fallo_de("ConvertStringSidToSidW"));
    }
    let sid = Local(sid);
    let testigo = abrir_mi_testigo(TOKEN_ADJUST_DEFAULT | TOKEN_QUERY)?;

    let etiqueta = TOKEN_MANDATORY_LABEL {
        Label: SID_AND_ATTRIBUTES {
            Sid: sid.0,
            Attributes: SE_GROUP_INTEGRITY as u32,
        },
    };
    // SAFETY: `sid.0` es un SID válido, el que acaba de crear `ConvertStringSidToSidW`.
    let largo_del_sid = unsafe { GetLengthSid(sid.0) };
    let largo = std::mem::size_of::<TOKEN_MANDATORY_LABEL>() as u32 + largo_del_sid;
    // SAFETY: `testigo` está abierto con `TOKEN_ADJUST_DEFAULT`; `etiqueta` y el SID al que apunta
    // viven durante la llamada, y `largo` es el de los dos juntos, como pide `TokenIntegrityLevel`.
    let cambiado = unsafe {
        SetTokenInformation(
            testigo.0,
            TokenIntegrityLevel,
            ptr::from_ref(&etiqueta).cast(),
            largo,
        )
    };
    if cambiado == 0 {
        return Err(fallo_de("SetTokenInformation"));
    }
    Ok(())
}

/// Pone a `carpeta` la etiqueta de integridad baja, heredable: lo que haya dentro y lo que se cree
/// después también la llevan.
pub fn marcar_carpeta_de_integridad_baja(carpeta: &Path) -> Resultado<()> {
    let sddl = ancho(OsStr::new(ETIQUETA_DE_INTEGRIDAD_BAJA));
    let mut descriptor = ptr::null_mut();
    // SAFETY: `sddl` termina en cero y vive durante la llamada; `descriptor` es una variable local
    // donde Windows deja un puntero que luego se libera con `LocalFree`. El tamaño no se pide.
    let convertido = unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            SDDL_REVISION_1,
            &mut descriptor,
            ptr::null_mut(),
        )
    };
    if convertido == 0 {
        return Err(fallo_de(
            "ConvertStringSecurityDescriptorToSecurityDescriptorW",
        ));
    }
    let descriptor = Local(descriptor);

    let mut presente = 0;
    let mut sacl: *mut ACL = ptr::null_mut();
    let mut por_defecto = 0;
    // SAFETY: `descriptor.0` es el descriptor válido que acaba de crear Windows; las tres salidas
    // son variables locales. `sacl` apunta dentro del descriptor, que vive hasta el final.
    let leido = unsafe {
        GetSecurityDescriptorSacl(descriptor.0, &mut presente, &mut sacl, &mut por_defecto)
    };
    if leido == 0 || presente == 0 {
        return Err(fallo_de("GetSecurityDescriptorSacl"));
    }

    let ruta = ancho(carpeta.as_os_str());
    darme_permiso_de_etiquetar(carpeta, &ruta)?;
    // SAFETY: `ruta` termina en cero y vive durante la llamada; `sacl` apunta dentro de
    // `descriptor`, que sigue vivo. Solo se cambia la etiqueta: propietario, grupo y DACL van nulos
    // y `LABEL_SECURITY_INFORMATION` dice que no se tocan.
    let codigo = unsafe {
        SetNamedSecurityInfoW(
            ruta.as_ptr(),
            SE_FILE_OBJECT,
            LABEL_SECURITY_INFORMATION,
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null(),
            sacl,
        )
    };
    sobre_la_carpeta(codigo, "SetNamedSecurityInfoW", carpeta)
}

/// Un Job Object: lo que se mete en él no puede pasar de `procesos` a la vez ni de `memoria` entre
/// todos, y muere entero cuando se suelta el `Trabajo`.
pub struct Trabajo {
    asa: Asa,
    /// El tope de procesos, que es también cuántos caben como mucho en la lista del Job.
    procesos_maximos: u32,
}

impl Trabajo {
    pub fn nuevo(procesos_maximos: u32, memoria_maxima_mib: u64) -> Resultado<Self> {
        let memoria = usize::try_from(memoria_maxima_mib)
            .ok()
            .and_then(|mib| mib.checked_mul(BYTES_POR_MIB))
            .ok_or_else(|| {
                Error::Aislamiento(format!(
                    "memoria_maxima_mib = {memoria_maxima_mib} no cabe en este sistema"
                ))
            })?;
        // SAFETY: sin atributos ni nombre, los dos nulos, como permite la función.
        let asa = unsafe { CreateJobObjectW(ptr::null(), ptr::null()) };
        if asa.is_null() {
            return Err(fallo_de("CreateJobObjectW"));
        }
        let trabajo = Self {
            asa: Asa(asa),
            procesos_maximos,
        };

        let mut limites = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limites.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
            | JOB_OBJECT_LIMIT_ACTIVE_PROCESS
            | JOB_OBJECT_LIMIT_JOB_MEMORY;
        limites.BasicLimitInformation.ActiveProcessLimit = procesos_maximos;
        limites.JobMemoryLimit = memoria;
        // SAFETY: el asa es la del Job recién creado; `limites` vive durante la llamada y el
        // tamaño es el de su tipo, el que corresponde a `JobObjectExtendedLimitInformation`.
        let puesto = unsafe {
            SetInformationJobObject(
                trabajo.asa.0,
                JobObjectExtendedLimitInformation,
                ptr::from_ref(&limites).cast(),
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        };
        if puesto == 0 {
            return Err(fallo_de("SetInformationJobObject"));
        }
        Ok(trabajo)
    }

    /// Mete al proceso en el Job. Lo que él lance a partir de ahora nace dentro.
    pub fn meter(&self, hijo: &Child) -> Resultado<()> {
        // SAFETY: las dos asas son válidas: la del Job la tiene `self`, y la del proceso, `hijo`,
        // que no ha sido esperado todavía.
        let metido =
            unsafe { AssignProcessToJobObject(self.asa.0, hijo.as_raw_handle() as HANDLE) };
        if metido == 0 {
            return Err(fallo_de("AssignProcessToJobObject"));
        }
        Ok(())
    }

    /// Mata todo lo que haya en el Job. Si no queda nada, no hace nada.
    pub fn cortar(&self) {
        // SAFETY: el asa del Job es válida mientras viva `self`.
        unsafe { TerminateJobObject(self.asa.0, CODIGO_AL_CORTAR) };
    }

    /// Los identificadores de los procesos que hay ahora mismo en el Job.
    pub fn procesos(&self) -> Resultado<Vec<u32>> {
        let palabra = std::mem::size_of::<usize>();
        let cabecera =
            std::mem::offset_of!(JOBOBJECT_BASIC_PROCESS_ID_LIST, ProcessIdList).div_ceil(palabra);
        let huecos = cabecera + self.procesos_maximos as usize;
        let mut lista = vec![0usize; huecos];
        // SAFETY: el asa del Job es válida mientras viva `self`; `lista` es un búfer de `huecos`
        // palabras, alineado como `usize`, que es lo que pide la estructura, y se pasa su tamaño
        // exacto en bytes. La longitud devuelta no se pide.
        let leida = unsafe {
            QueryInformationJobObject(
                self.asa.0,
                JobObjectBasicProcessIdList,
                lista.as_mut_ptr().cast(),
                (huecos * palabra) as u32,
                ptr::null_mut(),
            )
        };
        // Con más procesos de los que caben, Windows llena los que caben y avisa: valen igual.
        let llena = std::io::Error::last_os_error().raw_os_error() == Some(ERROR_MORE_DATA as i32);
        if leida == 0 && !llena {
            return Err(fallo_de("QueryInformationJobObject"));
        }
        // SAFETY: Windows escribió la cabecera al principio de `lista`, que tiene al menos ese
        // tamaño y la alineación de la estructura.
        let en_la_lista = unsafe {
            (*lista.as_ptr().cast::<JOBOBJECT_BASIC_PROCESS_ID_LIST>()).NumberOfProcessIdsInList
        };
        Ok(lista[cabecera..]
            .iter()
            .take(en_la_lista as usize)
            .map(|&id| id as u32)
            .collect())
    }
}

/// Los destinos de las conexiones TCP, de IPv4 y de IPv6, que tienen abiertas estos procesos.
pub fn conexiones_de(procesos: &[u32]) -> Resultado<Vec<SocketAddr>> {
    Ok(tabla_de_conexiones()?
        .into_iter()
        .filter(|(_, proceso)| procesos.contains(proceso))
        .map(|(remota, _)| remota)
        .collect())
}

/// Todas las conexiones TCP del equipo, de IPv4 y de IPv6: su destino y el proceso que la tiene.
pub fn tabla_de_conexiones() -> Resultado<Vec<(SocketAddr, u32)>> {
    let de_ipv4 = filas::<MIB_TCPROW_OWNER_PID>(&tabla_tcp(AF_INET)?)
        .into_iter()
        .map(|fila| {
            let ip = Ipv4Addr::from(fila.dwRemoteAddr.to_ne_bytes());
            let remota = SocketAddr::new(IpAddr::V4(ip), puerto(fila.dwRemotePort));
            (remota, fila.dwOwningPid)
        });
    let mut conexiones: Vec<_> = de_ipv4.collect();
    conexiones.extend(
        filas::<MIB_TCP6ROW_OWNER_PID>(&tabla_tcp(AF_INET6)?)
            .into_iter()
            .map(|fila| {
                let ip = Ipv6Addr::from(fila.ucRemoteAddr);
                let remota = SocketAddr::new(IpAddr::V6(ip), puerto(fila.dwRemotePort));
                (remota, fila.dwOwningPid)
            }),
    );
    Ok(conexiones)
}

/// El nombre del ejecutable de un proceso, como `rustc.exe`, o `None` si ya no existe o Windows no
/// deja preguntarlo. Basta el derecho de consulta limitada, que se tiene sobre los procesos de
/// integridad baja que lanza este.
pub fn nombre_del_proceso(proceso: u32) -> Option<String> {
    // SAFETY: `OpenProcess` no lee memoria nuestra; si falla devuelve un asa nula, que se mira
    // antes de envolverla.
    let asa = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, proceso) };
    if asa.is_null() {
        return None;
    }
    let asa = Asa(asa);
    let mut ruta = vec![0u16; LARGO_MAXIMO_DE_RUTA];
    let mut largo = LARGO_MAXIMO_DE_RUTA as u32;
    // SAFETY: el asa es válida mientras viva `asa`; `ruta` tiene `largo` caracteres y Windows
    // escribe dentro de ellos y deja en `largo` cuántos usó, sin contar el cero final.
    let leida = unsafe { QueryFullProcessImageNameW(asa.0, 0, ruta.as_mut_ptr(), &mut largo) };
    if leida == 0 {
        return None;
    }
    let ruta = String::from_utf16_lossy(&ruta[..largo as usize]);
    Path::new(&ruta)
        .file_name()
        .map(|nombre| nombre.to_string_lossy().into_owned())
}

/// El puerto de una fila de la tabla: los dos primeros bytes, en el orden de la red.
fn puerto(campo: u32) -> u16 {
    let [alto, bajo, _, _] = campo.to_ne_bytes();
    u16::from_be_bytes([alto, bajo])
}

/// La tabla TCP de una familia, con el proceso de cada conexión, en palabras de `u32`.
fn tabla_tcp(familia: u16) -> Resultado<Vec<u32>> {
    let mut bytes: u32 = 0;
    for _ in 0..INTENTOS_DE_LEER_LA_TABLA {
        let mut palabras = vec![0u32; (bytes as usize).div_ceil(BYTES_POR_PALABRA).max(1)];
        let mut disponibles = (palabras.len() * BYTES_POR_PALABRA) as u32;
        // SAFETY: `palabras` tiene `disponibles` bytes, alineados a `u32` como las filas; la
        // función escribe dentro de ellos o dice cuántos hacen falta en `disponibles`.
        let codigo = unsafe {
            GetExtendedTcpTable(
                palabras.as_mut_ptr().cast(),
                &mut disponibles,
                0,
                u32::from(familia),
                TCP_TABLE_OWNER_PID_ALL,
                0,
            )
        };
        match codigo {
            ERROR_SUCCESS => return Ok(palabras),
            ERROR_INSUFFICIENT_BUFFER => bytes = disponibles,
            otro => {
                return Err(Error::Aislamiento(format!(
                    "GetExtendedTcpTable: {}",
                    std::io::Error::from_raw_os_error(otro as i32)
                )))
            }
        }
    }
    Err(Error::Aislamiento(
        "la tabla de conexiones no paró de crecer mientras se leía".into(),
    ))
}

/// Las filas de una tabla TCP: la primera palabra dice cuántas hay, y van detrás.
///
/// Solo se usa con `MIB_TCPROW_OWNER_PID` y `MIB_TCP6ROW_OWNER_PID`, que son de `u32` y bytes
/// sueltos: cualquier contenido es un valor válido y su alineación es la de `u32`.
fn filas<T: Copy>(tabla: &[u32]) -> Vec<T> {
    let Some((&cuantas, resto)) = tabla.split_first() else {
        return Vec::new();
    };
    let caben = resto.len() * BYTES_POR_PALABRA / std::mem::size_of::<T>();
    let primera = resto.as_ptr().cast::<T>();
    (0..(cuantas as usize).min(caben))
        // SAFETY: `indice` es menor que las filas que caben en `resto`, así que la lectura queda
        // dentro del búfer; por lo dicho arriba, cualquier contenido es un `T` válido.
        .map(|indice| unsafe { primera.add(indice).read_unaligned() })
        .collect()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_nombre_de_un_proceso_es_el_de_su_ejecutable() {
        let propio = std::env::current_exe().unwrap();
        let esperado = propio.file_name().unwrap().to_string_lossy().into_owned();

        assert_eq!(nombre_del_proceso(std::process::id()), Some(esperado));
    }

    /// El 0 es el proceso inactivo del sistema, que no deja abrirse.
    #[test]
    fn de_un_proceso_que_no_se_deja_preguntar_no_hay_nombre() {
        assert_eq!(nombre_del_proceso(0), None);
    }

    /// El puerto 443 en una fila: `0x01BB` en el orden de la red, en los dos primeros bytes.
    #[test]
    fn el_puerto_de_una_fila_se_lee_en_el_orden_de_la_red() {
        assert_eq!(puerto(u32::from_ne_bytes([0x01, 0xBB, 0, 0])), 443);
    }

    #[test]
    fn las_filas_no_pasan_de_las_que_caben_aunque_la_cabecera_diga_mas() {
        let fila = [1u32, 2, 3, 4, 5, 6];
        let mut tabla = vec![9u32];
        tabla.extend(fila);

        let leidas = filas::<MIB_TCPROW_OWNER_PID>(&tabla);

        assert_eq!(leidas.len(), 1);
        assert_eq!(leidas[0].dwOwningPid, 6);
        assert!(filas::<MIB_TCPROW_OWNER_PID>(&[]).is_empty());
    }
}
