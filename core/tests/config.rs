//! Tests de integración de la persistencia de configuración y los perfiles
//! por dispositivo (`voxlfa_core::config`).

use std::collections::HashMap;
use std::path::PathBuf;

use voxlfa_core::config::{default_config_path, ConfigStore, DEFAULT_DEVICE_KEY};
use voxlfa_core::dsp::PresetFactory;
use voxlfa_core::protocol::{EqBand, EqBandKind, PresetId};

fn temp_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("voxlfa-config-{name}-{}", std::process::id()))
}

fn eq_band(gain_db: f32) -> EqBand {
    EqBand {
        kind: EqBandKind::Peaking,
        freq_hz: 3000.0,
        gain_db,
        q: 1.5,
    }
}

#[test]
fn default_preset_is_dry() {
    assert_eq!(PresetId::default(), PresetId::Dry);
}

#[test]
fn preset_factory_exposes_default_eq_bands() {
    assert_eq!(PresetFactory::eq_bands(PresetId::Dry).len(), 0);
    assert_eq!(PresetFactory::eq_bands(PresetId::VozLimpia).len(), 3);
    assert_eq!(PresetFactory::eq_bands(PresetId::Radio).len(), 2);
    assert_eq!(PresetFactory::eq_bands(PresetId::Warm).len(), 3);
}

#[test]
fn config_round_trip_preserves_profile() {
    let path = temp_path("roundtrip");
    {
        let mut store = ConfigStore::load(&path);
        {
            let config = store.config_mut();
            config.default_input = Some("Interfaz Scarlett 2i2".into());
            config.default_output = Some("Monitor 01".into());
            config.buffer_size = Some(128);
            let profile = config.profile_mut("Interfaz Scarlett 2i2");
            profile.preset = PresetId::Warm;
            profile.eq_bands = vec![eq_band(4.0)];
            profile.global_bypass = false;
            let mut link_bypass = HashMap::new();
            link_bypass.insert("reverb".to_string(), true);
            profile.link_bypass = link_bypass;
        }
        store.save().expect("guarda la configuración");
    }

    let store = ConfigStore::load(&path);
    let config = store.config();
    assert_eq!(
        config.default_input.as_deref(),
        Some("Interfaz Scarlett 2i2")
    );
    assert_eq!(config.buffer_size, Some(128));
    let profile = config
        .profile("Interfaz Scarlett 2i2")
        .expect("el perfil se guardó");
    assert_eq!(profile.preset, PresetId::Warm);
    assert_eq!(profile.eq_bands[0].gain_db, 4.0);
    assert_eq!(profile.link_bypass.get("reverb"), Some(&true));

    let _ = std::fs::remove_file(&path);
}

#[test]
fn default_device_key_profile_is_used_with_null_devices() {
    let path = temp_path("default-key");
    {
        let mut store = ConfigStore::load(&path);
        let profile = store.config_mut().profile_mut(DEFAULT_DEVICE_KEY);
        profile.preset = PresetId::Radio;
        store.save().expect("guarda");
    }
    let store = ConfigStore::load(&path);
    let profile = store
        .config()
        .profile(DEFAULT_DEVICE_KEY)
        .expect("perfil default");
    assert_eq!(profile.preset, PresetId::Radio);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn memory_store_does_not_write() {
    let mut store = ConfigStore::memory();
    store.config_mut().default_input = Some("x".into());
    // Sin ruta: guardar es un no-op, no debe fallar.
    store.save().expect("no-op sin fallo");
    assert_eq!(store.config().default_input.as_deref(), Some("x"));
}

#[test]
fn default_config_path_resolves_under_voxlfa_dir() {
    if let Some(path) = default_config_path() {
        assert_eq!(
            path.file_name().and_then(|s| s.to_str()),
            Some("config.json")
        );
        assert!(path.to_string_lossy().contains("voxlfa"));
    }
}
