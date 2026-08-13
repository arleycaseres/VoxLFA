//! Publicación del escritorio por mDNS (autodetección desde el móvil).
//!
//! VoxLFA se anuncia como `_voxlfa._tcp.local.` con la dirección LAN y el
//! puerto del WebSocket de monitoreo. Así la app móvil (u otras herramientas
//! como `avahi-browse` o `dns-sd`) puede descubrir la IP del escritorio sin
//! teclearla.
//!
//! # Seguridad
//! Los registros TXT solo llevan metadatos no sensibles (`name`, `ver`). El
//! código de emparejamiento **nunca** se publica por mDNS: es un secreto de
//! baja entropía y un hash suyo sería bruto-forzable offline; viaja solo por
//! el canal fuera de banda del QR (ver `docs/seguridad.md`).

use std::net::IpAddr;

use mdns_sd::{ServiceDaemon, ServiceInfo};

/// Tipo de servicio DNS-SD anunciado por el escritorio.
pub const SERVICE_TYPE: &str = "_voxlfa._tcp.local.";

/// Nombre de la instancia del servicio (visible para los navegadores).
pub const INSTANCE_NAME: &str = "VoxLFA";

/// Host al que apunta el registro SRV (target del servicio).
pub const SERVICE_HOST: &str = "voxlfa.local.";

/// Errores del anuncio mDNS.
#[derive(Debug, thiserror::Error)]
pub enum MdnsError {
    /// Fallo del crate `mdns-sd` (creación del daemon o registro del servicio).
    #[error("mDNS: {0}")]
    Mdns(#[from] mdns_sd::Error),
}

/// Anuncio mDNS del escritorio.
///
/// `ServiceDaemon` corre en su propio hilo; al soltar el anunciador se
/// detiene el daemon y se retiran los registros.
pub struct MdnsAdvertiser {
    daemon: ServiceDaemon,
}

impl MdnsAdvertiser {
    /// Publica el escritorio como `_voxlfa._tcp.local.` en la red local.
    ///
    /// - `addresses`: IPs LAN sobre las que anunciar (se añaden como registros
    ///   A del host).
    /// - `port`: puerto del WebSocket de monitoreo remoto.
    ///
    /// Devuelve el anunciador; si `mdns-sd` no puede crear el daemon o
    /// registrar el servicio, se obtiene un error (la app puede seguir
    /// funcionando sin autodetección).
    pub fn start(addresses: &[IpAddr], port: u16) -> Result<Self, MdnsError> {
        let daemon = ServiceDaemon::new()?;
        let properties = [("name", INSTANCE_NAME), ("ver", env!("CARGO_PKG_VERSION"))];
        let service = ServiceInfo::new(
            SERVICE_TYPE,
            INSTANCE_NAME,
            SERVICE_HOST,
            addresses,
            port,
            &properties[..],
        )?;
        daemon.register(service)?;
        Ok(Self { daemon })
    }
}

impl Drop for MdnsAdvertiser {
    fn drop(&mut self) {
        // Un fallo de apagado es inofensivo al final del ciclo de vida.
        let _ = self.daemon.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use std::net::IpAddr;
    use std::time::Duration;

    use mdns_sd::{ServiceDaemon, ServiceEvent};

    use super::{MdnsAdvertiser, INSTANCE_NAME, SERVICE_TYPE};

    /// Prefijo del dominio para el browse (el tipo + `.local.`).
    const BROWSE_TIMEOUT: Duration = Duration::from_secs(3);

    /// Tauri exige `AppState: Send + Sync`; verificar que el anunciador lo es
    /// en tiempo de compilación (falla aquí si `ServiceDaemon` no lo fuera).
    #[test]
    fn advertiser_is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<MdnsAdvertiser>();
    }

    #[test]
    fn advertise_then_browse_finds_the_service() {
        // Registrar un anuncio con el puerto del WS y una IP de bucle local.
        let port = 4356;
        let ip = IpAddr::from([127, 0, 0, 1]);
        let _advertiser =
            MdnsAdvertiser::start(&[ip], port).expect("el anuncio debe poder crearse");

        // Cliente DNS-SD independiente en el mismo proceso: buscar el servicio.
        let client = ServiceDaemon::new().expect("no se pudo crear el cliente mDNS");
        let receiver = client
            .browse(SERVICE_TYPE)
            .expect("no se pudo iniciar el browse mDNS");

        let mut found = false;
        let deadline = std::time::Instant::now() + BROWSE_TIMEOUT;
        while let Ok(event) =
            receiver.recv_timeout(deadline.saturating_duration_since(std::time::Instant::now()))
        {
            if let ServiceEvent::ServiceResolved(info) = event {
                assert_eq!(
                    info.get_fullname(),
                    format!("{INSTANCE_NAME}.{SERVICE_TYPE}")
                );
                assert_eq!(info.get_port(), port);
                assert!(
                    !info.get_addresses_v4().is_empty(),
                    "el servicio debe resolver al menos una dirección IPv4"
                );
                found = true;
                break;
            }
        }
        assert!(
            found,
            "el servicio _voxlfa._tcp.local. no se resolvió a tiempo"
        );
    }
}
