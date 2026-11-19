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
        c.poll_state_and_meters().unwrap();
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
        assert!((c.state().mixer_channels[i].level_db + 15.0).abs() < 0.25);
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
fn phantom_mask_in_266b_state_is_independent_of_other_flags() {
    let mut state = AudioFuseState::default();
    let mut packet = [0u8; AFEX_STATE_SIZE];
    packet[0..6].copy_from_slice(&AFEX_STATE_MAGIC);
    let phantom_off = (param_id::INPUT_PHANTOM as usize) * 2;
    packet[phantom_off..phantom_off + 2].copy_from_slice(&(1u16 << 7).to_le_bytes());
    state.apply_afex_state_266(&packet).unwrap();
    assert!(state.inputs[7].phantom_48v);
    assert!(!state.inputs[0].phantom_48v);

    packet[phantom_off..phantom_off + 2].copy_from_slice(&0u16.to_le_bytes());
    state.apply_afex_state_266(&packet).unwrap();
    assert!(!state.inputs[7].phantom_48v);
}

#[test]
fn all_output_pair_source_combinations_round_trip() {
    let mut c = simulator();
    for pair in 0..4 {
        for src in AnalogOutSource::ALL {
            c.set_analog_out_source(pair, src).unwrap();
            c.poll_state_and_meters().unwrap();
            assert_eq!(c.state().routing.analog_out_sources[pair], src);
        }
        for src in AdatOutSource::ALL {
            c.set_adat_out_source(pair, src).unwrap();
            c.poll_state_and_meters().unwrap();
            assert_eq!(c.state().routing.adat_out_sources[pair], src);
        }
    }
    for src in LoopbackSource::ALL {
        c.set_loopback_source(src).unwrap();
        c.poll_state_and_meters().unwrap();
        assert_eq!(c.state().routing.loopback_source, src);
    }
}

#[test]
fn one_hot_routing_masks_match_verified_hardware_wire_values() {
    assert_eq!(AnalogOutSource::CueMix.to_wire_mask(), 0x0001);
    assert_eq!(AnalogOutSource::UsbPair.to_wire_mask(), 0x0002);
    assert_eq!(AnalogOutSource::AnalogInPair.to_wire_mask(), 0x0004);
    assert_eq!(AnalogOutSource::AdatInPair.to_wire_mask(), 0x0008);

    assert_eq!(AdatOutSource::CueMix.to_wire_mask(), 0x0001);
    assert_eq!(AdatOutSource::UsbPair.to_wire_mask(), 0x0002);
    assert_eq!(AdatOutSource::AnalogInPair.to_wire_mask(), 0x0004);

    assert_eq!(LoopbackSource::Disabled.to_wire_mask(), 0x0000);
    assert_eq!(LoopbackSource::CueMix.to_wire_mask(), 0x0001);
    assert_eq!(LoopbackSource::Usb17_18.to_wire_mask(), 0x0002);
}

#[test]
fn non_simulator_transport_rejects_unsupported_controls_honestly() {
    struct StubHardwareTransport;

    impl AudioFuseTransport for StubHardwareTransport {
        fn backend_name(&self) -> &str {
            "stub-hw"
        }
        fn is_hardware_detected(&self) -> bool {
            true
        }
        fn read_state_266(&self) -> Result<[u8; AFEX_STATE_SIZE], TransportError> {
            let mut buf = [0u8; AFEX_STATE_SIZE];
            buf[0..6].copy_from_slice(&AFEX_STATE_MAGIC);
            Ok(buf)
        }
        fn read_vu_152(&self) -> Result<[u8; AFEX_VU_SIZE], TransportError> {
            Ok([0u8; AFEX_VU_SIZE])
        }
        fn get_parameter(&self, _param_id: u8, _channel_index: u8) -> Result<u16, TransportError> {
            Ok(0)
        }
        fn set_parameter(
            &self,
            _param_id: u8,
            _channel_index: u8,
            _value: u16,
        ) -> Result<(), TransportError> {
            Ok(())
        }
        fn set_parameter_bytes(
            &self,
            _param_id: u8,
            _channel_index: u8,
            _data: &[u8],
        ) -> Result<(), TransportError> {
            Ok(())
        }
        fn get_string(&self, _str_type: u8, _index: u16) -> Result<String, TransportError> {
            Ok("Audiofuse 8Pre".into())
        }
        fn set_string(
            &self,
            _str_type: u8,
            _index: u16,
            _value: &str,
        ) -> Result<(), TransportError> {
            Ok(())
        }
        fn get_clock_source(&self) -> Result<ClockSource, TransportError> {
            Ok(ClockSource::Internal)
        }
        fn set_clock_source(&self, _source: ClockSource) -> Result<(), TransportError> {
            Ok(())
        }
        fn get_clock_validity(&self) -> Result<[bool; 3], TransportError> {
            Ok([true, false, false])
        }
        fn get_sample_rate(&self) -> Result<SampleRate, TransportError> {
            Ok(SampleRate::default())
        }
        fn set_sample_rate(&self, _rate: SampleRate) -> Result<(), TransportError> {
            Ok(())
        }
        fn get_dfu_status(&self) -> Result<DfuStatusReport, TransportError> {
            Err(TransportError::Unsupported("DFU refused at runtime".into()))
        }
        fn discovery_info(&self) -> HardwareDiscoveryInfo {
            HardwareDiscoveryInfo::default()
        }
    }

    let t = Arc::new(StubHardwareTransport);
    assert!(matches!(
        t.set_simulated_mixer_output_mute(true),
        Err(TransportError::Unsupported(_))
    ));
    assert!(matches!(
        t.set_simulated_mixer_output_solo(true),
        Err(TransportError::Unsupported(_))
    ));
    assert!(matches!(
        t.set_simulated_direct_computer_mix(0.5),
        Err(TransportError::Unsupported(_))
    ));
    assert!(matches!(
        t.preset_load(0, 266),
        Err(TransportError::Unsupported(_))
    ));
    assert!(matches!(
        t.preset_save(0, &[0u8; 4]),
        Err(TransportError::Unsupported(_))
    ));

    let mut c = AudioFuseController::new(t).unwrap();
    assert!(c.set_direct_computer_mix(0.5).is_err());
    assert!(c.set_mixer_output_mute(true).is_err());
    assert!(c.set_mixer_output_solo(true).is_err());
    assert!(c.load_hardware_preset(0).is_err());
    assert!(c.save_hardware_preset(0).is_err());
    assert!(c.set_input_type(0, InputType::Line).is_err());
}
