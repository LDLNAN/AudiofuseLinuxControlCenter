//! `audiofuse-core`: Native USB/UAC2 control backend, state controller,
//! preset engine, and `.af8pre` firmware inspector for the Arturia AudioFuse 8Pre (`1c75:af0e`).

pub mod controller;
pub mod firmware;
pub mod preset;
pub mod protocol;
pub mod state;
pub mod transport;

pub use controller::{AudioFuseController, TelemetryFrame};
pub use firmware::{
    Af8PreImageEntry, Af8PreManifest, DfuState, DfuStatusReport, FirmwareError, FirmwareInspection,
    InspectedFirmwareImage, chunk_firmware_into_dfu_blocks, inspect_af8pre_bytes,
    inspect_af8pre_file,
};
pub use preset::{AudioFusePresetFile, PresetError};
pub use protocol::*;
pub use state::{
    AnalogInputChannel, AudioFuseState, BankStereoLinks, CueMixerChannel, DeviceConfig,
    MasterMonitorSection, RoutingMatrix,
};
pub use transport::{
    AudioFuseTransport, HardwareDiscoveryInfo, SimulatedTransport, TransportError,
    UsbHardwareTransport, open_best_available_transport, probe_linux_host_hardware,
};

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::Arc;

    #[test]
    fn test_af001_wvalue_and_windex_encoding() {
        assert_eq!(AFEX_DEFAULT_WINDEX, 0x0a00);
        assert_eq!(AFEX_STATE_WINDEX, 0x0d00);
        assert_eq!(AFEX_VU_WINDEX, 0x1100);
        for param_id in [
            param_id::INPUT_PAD_MIC,
            param_id::INPUT_PAD_LINE,
            param_id::INPUT_INST,
            param_id::INPUT_PHASE_INVERT,
            param_id::SPEAKERS_SOURCE,
            param_id::PHONES_SOURCE,
            param_id::SPEAKERS_MONO,
            param_id::PHONES_MONO,
            param_id::INPUT_PHANTOM,
            param_id::MIXER_CH_GAIN_L,
            param_id::MIXER_CH_GAIN_R,
            param_id::MIXER_MUTE_ANALOG_ADAT,
            param_id::MIXER_MUTE_USB,
            param_id::MIXER_SOLO_ANALOG_ADAT,
            param_id::MIXER_SOLO_USB,
            param_id::MIXER_OUT_LEVEL,
            param_id::MIXER_STEREO_SHOW,
            param_id::DEVICE_MODE,
            param_id::DEVICE_NAME,
            param_id::WORDCLOCK_IMPEDANCE,
            param_id::WORDCLOCK_THRU,
            param_id::ANALOG_OUT_SOURCE,
            param_id::ADAT_OUT_SOURCE,
            param_id::LOOPBACK_SOURCE,
            param_id::SMUX_MODE,
            param_id::ADAT_ON,
        ] {
            for idx in 0..24u8 {
                let wval = encode_param_wvalue(param_id, idx);
                assert_eq!(wval, ((param_id as u16) << 8) | (idx as u16));
                assert_eq!(decode_param_wvalue(wval), (param_id, idx));
            }
        }
    }

    #[test]
    fn test_af001_uac2_clock_and_sample_rate() {
        let sim = Arc::new(SimulatedTransport::default());
        let mut ctrl = AudioFuseController::new(sim.clone()).unwrap();

        // Verified USB descriptors & ALSA mapping:
        // Pin 1 -> Entity 41 (0x29) Internal, Pin 2 -> Entity 43 (0x2B) WORD, Pin 3 -> Entity 44 (0x2C) ADAT
        assert_eq!(ClockSource::Internal.to_uac2_pin(), 1);
        assert_eq!(ClockSource::WordClock.to_uac2_pin(), 2);
        assert_eq!(ClockSource::Adat.to_uac2_pin(), 3);
        assert_eq!(ClockSource::from_uac2_pin(1), ClockSource::Internal);
        assert_eq!(ClockSource::from_uac2_pin(2), ClockSource::WordClock);
        assert_eq!(ClockSource::from_uac2_pin(3), ClockSource::Adat);
        assert_eq!(ClockSource::Internal.uac2_clock_entity(), 0x29);
        assert_eq!(ClockSource::WordClock.uac2_clock_entity(), 0x2b);
        assert_eq!(ClockSource::Adat.uac2_clock_entity(), 0x2c);

        for sr in SampleRate::ALL {
            ctrl.set_sample_rate(sr).unwrap();
            assert_eq!(ctrl.state().config.sample_rate, sr);
            assert_eq!(sim.get_sample_rate().unwrap(), sr);
        }

        for cs in [
            ClockSource::Internal,
            ClockSource::WordClock,
            ClockSource::Adat,
        ] {
            ctrl.set_clock_source(cs).unwrap();
            assert_eq!(ctrl.state().config.clock_source, cs);
            assert_eq!(sim.get_clock_source().unwrap(), cs);
        }
    }

    #[test]
    fn test_af002_analog_input_channel_strips() {
        let sim = Arc::new(SimulatedTransport::default());
        let mut ctrl = AudioFuseController::new(sim.clone()).unwrap();

        // Channels 1 & 2 support Inst Hi-Z
        ctrl.set_input_inst(0, true).unwrap();
        assert!(ctrl.state().inputs[0].inst_hi_z);
        assert_eq!(ctrl.state().inputs[0].input_type, InputType::Inst);
        assert_eq!(sim.get_parameter(param_id::INPUT_INST, 0).unwrap(), 1);

        // Channels 3..8 reject Inst Hi-Z and clamp to Line in simulator
        ctrl.set_input_type(2, InputType::Inst).unwrap();
        assert_eq!(ctrl.state().inputs[2].input_type, InputType::Line);
        assert!(!ctrl.state().inputs[2].inst_hi_z);

        // +48V Phantom power bank 1-4 and 5-8
        ctrl.set_phantom_bank_4(0, true).unwrap();
        for ch in 0..4 {
            assert!(ctrl.state().inputs[ch].phantom_48v);
            assert_eq!(
                sim.get_parameter(param_id::INPUT_PHANTOM, ch as u8)
                    .unwrap(),
                1
            );
        }
        for ch in 4..8 {
            assert!(!ctrl.state().inputs[ch].phantom_48v);
        }

        // Phase invert, Line Pad (-20dB), and Mic Pad/Boost
        ctrl.set_input_type(5, InputType::ComboAuto).unwrap();
        ctrl.set_input_phase_invert(5, true).unwrap();
        ctrl.set_input_line_pad(5, LinePad::Pad20dB).unwrap();
        ctrl.set_input_mic_pad_boost(5, MicPadBoost::Boost).unwrap();
        assert!(ctrl.state().inputs[5].phase_invert);
        assert_eq!(ctrl.state().inputs[5].pad_line, LinePad::Pad20dB);
        assert_eq!(ctrl.state().inputs[5].pad_mic, MicPadBoost::Boost);
    }

    #[test]
    fn test_af003_piecewise_linear_fader_and_q88_constant_power_pan() {
        // Knee at 0.842105263 -> 0.0 dB
        assert!((fader_normalized_to_db(0.0) - (-61.0)).abs() < 1e-4);
        assert!((fader_normalized_to_db(MIXER_FADER_KNEE) - 0.0).abs() < 1e-4);
        assert!((fader_normalized_to_db(1.0) - 12.0).abs() < 1e-4);

        assert!((fader_db_to_normalized(-61.0) - 0.0).abs() < 1e-4);
        assert!((fader_db_to_normalized(0.0) - MIXER_FADER_KNEE).abs() < 1e-4);
        assert!((fader_db_to_normalized(12.0) - 1.0).abs() < 1e-4);

        // Constant-power Q8.8 pan law round-trip matching real hardware registers 0x20 / 0x38
        let (l_c, r_c) = encode_mixer_channel_gains_q88(0.0, 0);
        assert!((l_c - (-771)).abs() <= 3 && (r_c - (-771)).abs() <= 3);
        let (db_c, pan_c) = decode_mixer_channel_gains_q88(l_c, r_c);
        assert!((db_c - 0.0).abs() <= 0.1 && pan_c == 0);

        let (l_hl, r_hl) = encode_mixer_channel_gains_q88(0.0, -64);
        assert_eq!(l_hl, 0);
        assert_eq!(r_hl, i16::MIN);
        assert_eq!(decode_mixer_channel_gains_q88(l_hl, r_hl), (0.0, -64));

        let (l_hr, r_hr) = encode_mixer_channel_gains_q88(0.0, 64);
        assert_eq!(l_hr, i16::MIN);
        assert_eq!(r_hr, 0);
        assert_eq!(decode_mixer_channel_gains_q88(l_hr, r_hr), (0.0, 64));

        assert_eq!(format_db(-61.0), "-inf dB");
        assert_eq!(format_db(-60.6), "-inf dB");
        assert_eq!(format_db(0.0), "0.0 dB");
        assert_eq!(format_db(6.5), "+6.5 dB");
        assert_eq!(format_pan(-64), "<64");
        assert_eq!(format_pan(0), "C");
        assert_eq!(format_pan(64), "64>");

        let sim = Arc::new(SimulatedTransport::default());
        let mut ctrl = AudioFuseController::new(sim.clone()).unwrap();

        // Stereo link Analog 1-2: odd pan = -64, even pan = +64, level/mute/solo linked
        ctrl.set_stereo_link(MixerBank::Analog, 0, true).unwrap();
        assert_eq!(ctrl.state().mixer_channels[0].pan, -64);
        assert_eq!(ctrl.state().mixer_channels[1].pan, 64);

        ctrl.set_mixer_channel_level_db(0, -6.0).unwrap();
        assert!((ctrl.state().mixer_channels[0].level_db - (-6.0)).abs() <= 0.2);
        assert_eq!(
            ctrl.state().mixer_channels[0].level_db,
            ctrl.state().mixer_channels[1].level_db
        );

        // Channel Group 1 across ADAT 1 (idx 8) and USB 1 (idx 16)
        ctrl.set_mixer_channel_group(8, 1);
        ctrl.set_mixer_channel_group(16, 1);
        ctrl.set_mixer_channel_mute(8, true).unwrap();
        assert!(ctrl.state().mixer_channels[8].mute);
        assert!(ctrl.state().mixer_channels[16].mute);

        // 16-char custom channel naming in simulator
        ctrl.set_mixer_channel_name(0, "Kick Drum Sub In 12345")
            .unwrap();
        assert_eq!(ctrl.state().mixer_channels[0].name.len(), 16);
        assert_eq!(ctrl.state().mixer_channels[0].name, "Kick Drum Sub In");
        assert_eq!(
            sim.get_string(STRING_TYPE_CHANNEL_NAME, 0).unwrap(),
            "Kick Drum Sub In"
        );
    }

    #[test]
    fn test_af004_master_monitor_and_output_routing_matrix() {
        let sim = Arc::new(SimulatedTransport::default());
        let mut ctrl = AudioFuseController::new(sim).unwrap();

        ctrl.set_direct_computer_mix(0.25).unwrap();
        ctrl.set_mixer_output_level_db(-12.0).unwrap();
        ctrl.set_mixer_output_mute(true).unwrap();
        ctrl.set_speakers_source(SpeakersSource::CueMix).unwrap();
        ctrl.set_speakers_mono(true).unwrap();
        ctrl.set_phones_source(PhonesSource::FollowSpeakers)
            .unwrap();
        ctrl.set_phones_mono(true).unwrap();

        assert!((ctrl.state().monitor.direct_computer_mix - 0.25).abs() < 0.02);
        assert!((ctrl.state().monitor.mixer_output_level_db - (-12.0)).abs() <= 0.2);
        assert!(ctrl.state().monitor.mixer_output_mute);
        assert_eq!(ctrl.state().monitor.speakers_source, SpeakersSource::CueMix);
        assert!(ctrl.state().monitor.speakers_mono);
        assert_eq!(
            ctrl.state().monitor.phones_source,
            PhonesSource::FollowSpeakers
        );
        assert!(ctrl.state().monitor.phones_mono);

        ctrl.set_analog_out_source(0, AnalogOutSource::CueMix)
            .unwrap();
        ctrl.set_analog_out_source(1, AnalogOutSource::AnalogInPair)
            .unwrap();
        ctrl.set_analog_out_source(2, AnalogOutSource::AdatInPair)
            .unwrap();
        ctrl.set_adat_out_source(0, AdatOutSource::AnalogInPair)
            .unwrap();
        ctrl.set_loopback_source(LoopbackSource::Usb17_18).unwrap();

        assert_eq!(
            ctrl.state().routing.analog_out_sources[0],
            AnalogOutSource::CueMix
        );
        assert_eq!(
            ctrl.state().routing.analog_out_sources[1],
            AnalogOutSource::AnalogInPair
        );
        assert_eq!(
            ctrl.state().routing.analog_out_sources[2],
            AnalogOutSource::AdatInPair
        );
        assert_eq!(
            ctrl.state().routing.adat_out_sources[0],
            AdatOutSource::AnalogInPair
        );
        assert_eq!(
            ctrl.state().routing.loopback_source,
            LoopbackSource::Usb17_18
        );
    }

    #[test]
    fn test_af005_verified_266b_state_and_152b_vu_roundtrip() {
        let mut state = AudioFuseState::default();
        state.config.device_name = "Studio 8Pre".to_string();
        state.config.device_mode = DeviceMode::Adat;
        state.config.wordclock_impedance = WordclockImpedance::HiZ;
        state.config.wordclock_thru = true;
        state.config.smux_mode = SmuxMode::DoubleSpeed;
        state.config.adat_on = false;
        state.inputs[0].phantom_48v = true;
        state.inputs[0].inst_hi_z = true;
        state.inputs[0].input_type = InputType::Inst;
        state.inputs[3].phase_invert = true;
        state.inputs[3].pad_line = LinePad::Pad20dB;
        state.inputs[3].pad_mic = MicPadBoost::Boost;
        state.mixer_channels[5].level_db = -9.5;
        state.mixer_channels[5].pan = -32;
        state.mixer_channels[5].mute = true;
        state.mixer_channels[12].solo = true;
        state.mixer_channels[18].mute = true;
        state.stereo_links[2].pair_1_2 = true;
        state.monitor.mixer_output_level_db = -6.0;
        state.monitor.speakers_source = SpeakersSource::CueMix;
        state.monitor.speakers_mono = true;
        state.monitor.phones_source = PhonesSource::FollowSpeakers;
        state.monitor.phones_mono = true;
        state.routing.analog_out_sources[0] = AnalogOutSource::AdatInPair;
        state.routing.adat_out_sources[1] = AdatOutSource::AnalogInPair;
        state.routing.loopback_source = LoopbackSource::Usb17_18;

        let encoded = state.encode_afex_state_266();
        assert_eq!(encoded.len(), AFEX_STATE_SIZE);
        assert_eq!(&encoded[0..6], &AFEX_STATE_MAGIC);

        let mut decoded = AudioFuseState::default();
        decoded.apply_afex_state_266(&encoded).unwrap();

        assert_eq!(decoded.config.device_name, "Studio 8Pre");
        assert_eq!(decoded.config.device_mode, DeviceMode::Adat);
        assert_eq!(decoded.config.wordclock_impedance, WordclockImpedance::HiZ);
        assert!(decoded.config.wordclock_thru);
        assert_eq!(decoded.config.smux_mode, SmuxMode::DoubleSpeed);
        assert!(!decoded.config.adat_on);
        assert!(decoded.inputs[0].phantom_48v);
        assert!(decoded.inputs[0].inst_hi_z);
        assert!(decoded.inputs[3].phase_invert);
        assert_eq!(decoded.inputs[3].pad_line, LinePad::Pad20dB);
        assert_eq!(decoded.inputs[3].pad_mic, MicPadBoost::Boost);
        assert!((decoded.mixer_channels[5].level_db - (-9.5)).abs() <= 0.15);
        assert!((decoded.mixer_channels[5].pan - (-32)).abs() <= 1);
        assert!(decoded.mixer_channels[5].mute);
        assert!(decoded.mixer_channels[12].solo);
        assert!(decoded.mixer_channels[18].mute);
        assert!(decoded.stereo_links[2].pair_1_2);
        assert!((decoded.monitor.mixer_output_level_db - (-6.0)).abs() <= 0.1);
        assert_eq!(decoded.monitor.speakers_source, SpeakersSource::CueMix);
        assert!(decoded.monitor.speakers_mono);
        assert_eq!(
            decoded.monitor.phones_source,
            PhonesSource::FollowSpeakers
        );
        assert!(decoded.monitor.phones_mono);
        assert_eq!(
            decoded.routing.analog_out_sources[0],
            AnalogOutSource::AdatInPair
        );
        assert_eq!(
            decoded.routing.adat_out_sources[1],
            AdatOutSource::AnalogInPair
        );
        assert_eq!(
            decoded.routing.loopback_source,
            LoopbackSource::Usb17_18
        );

        // Verify 152-byte live VU packet decoding & routing derivation
        let mut vu_buf = [0u8; AFEX_VU_SIZE];
        let full_scale_24: i32 = 8_388_607 << 8; // 0 dBFS -> 127
        let half_scale_24: i32 = 4_194_304 << 8; // ~ -6 dBFS -> ~114
        vu_buf[0..4].copy_from_slice(&half_scale_24.to_le_bytes()); // Analog In 1
        vu_buf[36 * 4..37 * 4].copy_from_slice(&full_scale_24.to_le_bytes()); // Cue Mix L
        vu_buf[37 * 4..38 * 4].copy_from_slice(&half_scale_24.to_le_bytes()); // Cue Mix R
        decoded.apply_afex_vu_152(&vu_buf);
        assert!(decoded.inputs[0].vu >= 112 && decoded.inputs[0].vu <= 116);
        assert_eq!(decoded.monitor.mixer_main_vu_l, 127);
        assert_eq!(decoded.monitor.speakers_vu_l, 127); // Speakers = CueMix
        assert_eq!(decoded.monitor.phones_vu_l, 127); // Phones = FollowSpeakers
    }

    #[test]
    fn test_af006_presets_and_firmware_inspector() {
        let sim = Arc::new(SimulatedTransport::default());
        let mut ctrl = AudioFuseController::new(sim).unwrap();

        ctrl.set_sample_rate(SampleRate::Rate88200).unwrap();
        ctrl.set_smux_mode(SmuxMode::DoubleSpeed).unwrap();
        ctrl.set_input_phantom(1, true).unwrap();
        ctrl.set_mixer_channel_level_db(3, 4.5).unwrap();
        ctrl.save_hardware_preset(2).unwrap();

        // Change values, then reload slot 2
        ctrl.set_sample_rate(SampleRate::Rate44100).unwrap();
        ctrl.set_input_phantom(1, false).unwrap();
        ctrl.load_hardware_preset(2).unwrap();
        assert_eq!(ctrl.state().config.sample_rate, SampleRate::Rate88200);
        assert_eq!(ctrl.state().config.smux_mode, SmuxMode::DoubleSpeed);
        assert!(ctrl.state().inputs[1].phantom_48v);
        assert!((ctrl.state().mixer_channels[3].level_db - 4.5).abs() <= 0.2);

        // Build an in-memory .af8pre ZIP archive and inspect it
        let mut zip_buf = Vec::new();
        {
            let cursor = std::io::Cursor::new(&mut zip_buf);
            let mut writer = zip::ZipWriter::new(cursor);
            let options = zip::write::SimpleFileOptions::default();
            writer.start_file("info.json", options).unwrap();
            writer
                .write_all(
                    br#"{
                        "date": "2020_10_20",
                        "vendorid": "0x1C75",
                        "version_number": "1.0.4",
                        "images": [
                            {
                                "file_name": "audiofuse_expander__fw1_0_4_8385329__2020_10_20.bin",
                                "image_num": 0
                            }
                        ],
                        "productid": "0xAF0E"
                    }"#,
                )
                .unwrap();
            writer
                .start_file(
                    "audiofuse_expander__fw1_0_4_8385329__2020_10_20.bin",
                    options,
                )
                .unwrap();
            writer.write_all(&vec![0xAAu8; 64256]).unwrap();
            writer.finish().unwrap();
        }

        let inspected = inspect_af8pre_bytes(&zip_buf, "test.af8pre").unwrap();
        assert!(inspected.targets_audiofuse_8pre);
        assert_eq!(inspected.manifest.version_number, "1.0.4");
        assert_eq!(inspected.images.len(), 1);
        assert_eq!(inspected.images[0].size_bytes, 64256);
        assert_eq!(inspected.images[0].dfu_block_count_64b, 1004);
    }
}
