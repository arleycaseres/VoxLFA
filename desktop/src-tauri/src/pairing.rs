//! Código de emparejamiento para la conexión desktop ↔ móvil.
//!
//! El escritorio genera un código corto que la app móvil debe teclear (o leer
//! de un QR en el futuro). La conexión WebSocket se rechaza sin ese token, de
//! modo que un dispositivo cualquiera en la misma WiFi no pueda controlar el
//! motor durante un evento en vivo.

use rand::Rng;

/// Alfabeto sin caracteres ambiguos (sin `I`, `O`, `0`, `1`) para que el
/// código sea fácil de leer y teclear.
const ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";

/// Longitud del código por defecto.
pub const DEFAULT_CODE_LENGTH: usize = 6;

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
}
