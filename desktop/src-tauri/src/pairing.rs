//! Código de emparejamiento para la conexión desktop ↔ móvil.
//!
//! El escritorio genera un código corto que la app móvil debe teclear (o leer
//! de un QR en el futuro). La conexión WebSocket se rechaza sin ese token, de
//! modo que un dispositivo cualquiera en la misma WiFi no pueda controlar el
//! motor durante un evento en vivo.
//!
//! El código **rota automáticamente** tras `MAX_FAILED_ATTEMPTS` intentos de
//! handshake fallidos consecutivos (mitiga fuerza bruta en la red local): el
//! nuevo código se publica para que la cabina lo muestre.

use rand::Rng;

/// Alfabeto sin caracteres ambiguos (sin `I`, `O`, `0`, `1`) para que el
/// código sea fácil de leer y teclear.
const ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";

/// Longitud del código por defecto.
pub const DEFAULT_CODE_LENGTH: usize = 6;

/// Intentos de handshake fallidos consecutivos antes de rotar el código.
pub const MAX_FAILED_ATTEMPTS: usize = 3;

/// Resultado de intentar autenticar un candidato contra el código actual.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthResult {
    /// El candidato coincide con el código actual (el contador se reinicia).
    Accepted,
    /// No coincide; se registró un intento fallido.
    Rejected,
    /// No coincide y además se rotó el código (ya hay uno nuevo publicado).
    RejectedAndRotated,
}

/// Estado compartido del código de emparejamiento con rotación automática.
///
/// Se comparte entre el servidor WebSocket (que autentica cada handshake) y la
/// cabina (que muestra el código vigente). Un solo `Mutex` protege el código y
/// el contador de fallos: las decisiones de autenticación son atómicas.
pub struct PairingState {
    code: String,
    failed_attempts: usize,
}

impl PairingState {
    /// Crea el estado con un código aleatorio de la longitud indicada.
    pub fn new(code_length: usize) -> Self {
        Self {
            code: generate_pairing_code(code_length),
            failed_attempts: 0,
        }
    }

    /// Crea el estado con un código prefijado (solo para tests).
    pub fn with_code(code: String) -> Self {
        Self {
            code,
            failed_attempts: 0,
        }
    }

    /// Código vigente (para mostrarlo en la cabina).
    pub fn code(&self) -> &str {
        &self.code
    }

    /// Autentica un candidato contra el código vigente.
    ///
    /// Un acierto reinicia el contador de fallos. Un fallo lo incrementa y, si
    /// se llega al máximo permitido, rota el código al instante (el candidato
    /// se rechaza igualmente).
    pub fn authenticate(&mut self, candidate: Option<&str>) -> AuthResult {
        let valid = candidate.is_some_and(|candidate| candidate == self.code);
        if valid {
            self.failed_attempts = 0;
            return AuthResult::Accepted;
        }
        self.failed_attempts += 1;
        if self.failed_attempts >= MAX_FAILED_ATTEMPTS {
            self.rotate();
            AuthResult::RejectedAndRotated
        } else {
            AuthResult::Rejected
        }
    }

    /// Regenera el código y reinicia el contador de fallos.
    pub fn rotate(&mut self) {
        self.code = generate_pairing_code(self.code.len());
        self.failed_attempts = 0;
    }
}

/// Genera un código de emparejamiento aleatorio de la longitud indicada.
///
/// # Seguridad
/// Con `length = 6` hay `32^6 ≈ 10^9` combinaciones: suficiente para una red
/// local; el token nunca se loguea y solo se muestra en la UI.
pub fn generate_pairing_code(length: usize) -> String {
    debug_assert!(length > 0, "el código de emparejamiento no puede ser vacío");
    let mut rng = rand::thread_rng();
    let mut code = String::with_capacity(length);
    for _ in 0..length {
        let index = rng.gen_range(0..ALPHABET.len());
        code.push(ALPHABET[index] as char);
    }
    code
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_has_requested_length() {
        assert_eq!(generate_pairing_code(6).len(), 6);
        assert_eq!(generate_pairing_code(8).len(), 8);
    }

    #[test]
    fn code_only_uses_safe_alphabet() {
        let code = generate_pairing_code(32);
        for byte in code.bytes() {
            assert!(
                ALPHABET.contains(&byte),
                "carácter inesperado en el código: {byte}"
            );
        }
    }

    #[test]
    fn two_codes_are_varied() {
        let a = generate_pairing_code(6);
        let b = generate_pairing_code(6);
        // No hay garantía matemática de que difieran, pero es prácticamente
        // seguro; este test detecta generadores degenerados.
        assert_ne!(a, b);
    }

    #[test]
    fn correct_code_is_accepted_and_resets_counter() {
        let mut state = PairingState::with_code("ABC234".into());
        // Dos fallos consecutivos no rotan todavía.
        assert_eq!(state.authenticate(Some("WRONG1")), AuthResult::Rejected);
        assert_eq!(state.authenticate(Some("WRONG2")), AuthResult::Rejected);
        // Un acierto reinicia el contador.
        assert_eq!(state.authenticate(Some("ABC234")), AuthResult::Accepted);
        // Un solo fallo después del acierto no debe rotar.
        assert_eq!(state.authenticate(Some("WRONG3")), AuthResult::Rejected);
        assert_eq!(state.code(), "ABC234");
    }

    #[test]
    fn missing_token_counts_as_failure() {
        let mut state = PairingState::with_code("ABC234".into());
        assert_eq!(state.authenticate(None), AuthResult::Rejected);
        assert_eq!(state.authenticate(None), AuthResult::Rejected);
    }

    #[test]
    fn code_rotates_after_max_failed_attempts() {
        let mut state = PairingState::with_code("ABC234".into());
        assert_eq!(state.authenticate(Some("NOPE1")), AuthResult::Rejected);
        assert_eq!(state.authenticate(Some("NOPE2")), AuthResult::Rejected);
        // Tercer fallo: rota y rechaza.
        let result = state.authenticate(Some("NOPE3"));
        assert_eq!(result, AuthResult::RejectedAndRotated);
        assert_ne!(state.code(), "ABC234");
        // El código nuevo vuelve a admitir `MAX` fallos antes de rotar.
        assert_eq!(state.authenticate(Some("NOPE4")), AuthResult::Rejected);
    }

    #[test]
    fn rotation_is_explicit_and_resets_counter() {
        let mut state = PairingState::with_code("ABC234".into());
        state.rotate();
        assert_ne!(state.code(), "ABC234");
        assert_eq!(state.authenticate(Some("NOPE")), AuthResult::Rejected);
    }
}
