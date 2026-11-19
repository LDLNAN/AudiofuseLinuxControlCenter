use audiofuse_core::*;
use std::sync::Arc;

fn simulator() -> AudioFuseController {
    AudioFuseController::new(Arc::new(SimulatedTransport::default())).unwrap()
}

#[test]
fn input_mode_rejects_incompatible_pad_and_tracks_connectors() {
    let mut c = simulator();
    for i in 0..8 {
        c.set_input_type(i, InputType::Mic).unwrap();
        c.set_input_mic_pad_boost(i, MicPadBoost::Boost).unwrap();
        assert!(c.set_input_line_pad(i, LinePad::Pad20dB).is_err());
        if i < 2 {
            c.set_input_inst(i, true).unwrap();
        } else {
            c.set_input_type(i, InputType::Line).unwrap();
        }
        c.poll_bulk_state_and_meters().unwrap();
        let input = &c.state().inputs[i];
        assert!(!input.mic_active && input.jack_active);
        assert_eq!(input.pad_mic, MicPadBoost::Off);
        assert!(c.set_input_mic_pad_boost(i, MicPadBoost::Boost).is_err());
    }
}

#[test]
fn relative_db_move_preserves_linked_group_pair() {
    let mut c = simulator();
    c.set_mixer_channel_level_db(0, -6.0).unwrap();
    c.set_mixer_channel_level_db(8, -18.0).unwrap();
    c.set_mixer_channel_group(0, 2);
    c.set_mixer_channel_group(8, 2);
    c.set_stereo_link(MixerBank::Adat, 0, true).unwrap();
    c.set_mixer_channel_level_db(0, -3.0).unwrap();
    for i in [8, 9] {
        assert!((c.state().mixer_channels[i].level_db + 15.0).abs() < 0.15);
    }
}

#[test]
fn simulator_names_round_trip_every_slot_and_channel() {
    let mut c = simulator();
    for slot in 0..8 {
        c.set_device_name(&format!("Rack {slot}")).unwrap();
        for i in 0..24 {
            c.set_mixer_channel_name(i, &format!("Ch {slot}-{i}"))
                .unwrap();
        }
        c.save_hardware_preset(slot).unwrap();
    }
    for slot in 0..8 {
        c.load_hardware_preset(slot).unwrap();
        assert_eq!(c.state().config.device_name, format!("Rack {slot}"));
        for i in 0..24 {
            assert_eq!(c.state().mixer_channels[i].name, format!("Ch {slot}-{i}"));
        }
    }
}

#[test]
fn snapshot_validation_precedes_mutation() {
    let mut c = simulator();
    let before = c.state().clone();
    let mut bad = before.clone();
    bad.config.device_name = "Changed".into();
    bad.mixer_channels.pop();
    assert!(c.apply_full_state(bad).is_err());
    assert_eq!(c.state(), &before);
    let preset = AudioFusePresetFile::from_state("test", "", before);
    for (key, value) in [
        ("schema_version", serde_json::json!(999)),
        ("product_id", serde_json::json!("0x0001")),
        ("vendor_id", serde_json::json!("0x0000")),
    ] {
        let mut json = serde_json::to_value(&preset).unwrap();
        json[key] = value;
        assert!(AudioFusePresetFile::from_json_str(&json.to_string()).is_err());
    }
}

#[test]
fn phantom_mask_is_independent_of_clock_and_master_flags() {
    let mut state = AudioFuseState::default();
    let mut packet = [0; 78];
    packet[53] = 128;
    state.apply_bulk_state_78(&packet);
    assert!(state.inputs[7].phantom_48v);
    state.apply_bulk_state_78(&[0; 78]);
    assert!(!state.inputs[7].phantom_48v);
}

#[test]
fn all_output_pair_source_combinations_round_trip() {
    let mut c = simulator();
    for pair in 0..4 {
        for src in AnalogOutSource::ALL {
            c.set_analog_out_source(pair, src).unwrap();
            c.poll_bulk_state_and_meters().unwrap();
            assert_eq!(c.state().routing.analog_out_sources[pair], src);
        }
        for src in AdatOutSource::ALL {
            c.set_adat_out_source(pair, src).unwrap();
            c.poll_bulk_state_and_meters().unwrap();
            assert_eq!(c.state().routing.adat_out_sources[pair], src);
        }
    }
    for src in LoopbackSource::ALL {
        c.set_loopback_source(src).unwrap();
        c.poll_bulk_state_and_meters().unwrap();
        assert_eq!(c.state().routing.loopback_source, src);
    }
}
