/* SPDX-License-Identifier: GPL-2.0-only */
/*
 * Strict AudioFuse 8Pre (1c75:af0e) EP0 Interface 0 transfer policy.
 *
 * Shared between kmod/audiofuse8pre_ctl.c (kernel) and kmod/test_policy.c (unit tests).
 */
#ifndef AUDIOFUSE8PRE_POLICY_H
#define AUDIOFUSE8PRE_POLICY_H

#ifdef __KERNEL__
#include <linux/types.h>
#else
#include <stdbool.h>
#include <stdint.h>
typedef uint8_t u8;
typedef uint16_t u16;
typedef uint32_t u32;
#endif

#include "audiofuse8pre_ctl.h"

#define AF8PRE_REQ_IN  0xa1
#define AF8PRE_REQ_OUT 0x21

#define AF8PRE_UAC2_CUR   0x01
#define AF8PRE_UAC2_RANGE 0x02
#define AF8PRE_UAC2_MEM   0x03

static inline u16 af8pre_le16(const u8 *data)
{
	return (u16)data[0] | ((u16)data[1] << 8);
}

static inline u32 af8pre_le32(const u8 *data)
{
	return (u32)data[0] |
	       ((u32)data[1] << 8) |
	       ((u32)data[2] << 16) |
	       ((u32)data[3] << 24);
}

/*
 * Validate an EP0 Class Interface control request header against the verified
 * AudioFuse 8Pre (1c75:af0e) Interface 0 protocol specification.
 *
 * Returns true ONLY if:
 *  - interface == 0x00 (Interfaces 1/2 streaming and 3 DFU are always rejected)
 *  - bm_request_type is 0xa1 (IN) or 0x21 (OUT)
 *  - OUT transfers require file_has_write == true (FMODE_WRITE)
 *  - entity, b_request, w_value (param_id/selector + channel), and w_length
 *    match an explicitly documented supported operation.
 */
static inline bool af8pre_policy_allows_header(u8 bm_request_type,
					       u8 b_request,
					       u16 w_value,
					       u16 w_index,
					       u16 w_length,
					       bool file_has_write, bool can_retune)
{
	u8 iface = w_index & 0xff;
	u8 entity = (w_index >> 8) & 0xff;
	bool is_in;
	bool is_out;

	if (bm_request_type == AF8PRE_REQ_IN) {
		is_in = true;
		is_out = false;
	} else if (bm_request_type == AF8PRE_REQ_OUT) {
		is_in = false;
		is_out = true;
	} else {
		return false;
	}

	/* Require write open mode (O_WRONLY / O_RDWR) for all OUT transfers. */
	if (is_out && (!file_has_write || ((entity == 0x28 || entity == 0x29) && !can_retune)))
		return false;

	/* HARD SAFETY RULE: Interface 0 (AudioControl) ONLY. */
	if (iface != 0x00)
		return false;

	if (w_length == 0 || w_length > AF8PRE_MAX_DATA_LEN)
		return false;

	switch (entity) {
	case 0x0d: /* AFEX bulk state snapshot unit (READ-ONLY) */
		if (!is_in || b_request != AF8PRE_UAC2_MEM)
			return false;
		if (w_value == 0x0002 && w_length == 266)
			return true;
		if (w_value == 0x0000 && w_length == 56)
			return true;
		if (w_value == 0x0001 && w_length == 204)
			return true;
		return false;

	case 0x11: /* AFEX live VU meter telemetry unit (READ-ONLY, 152 bytes = 38 x i32) */
		return is_in &&
		       b_request == AF8PRE_UAC2_MEM &&
		       w_value == 0x0000 &&
		       w_length == 152;

	case 0x0a: /* AFEX parameter control unit */ {
		u8 param_id = (w_value >> 8) & 0xff;
		u8 ch = w_value & 0xff;

		if (b_request != AF8PRE_UAC2_MEM)
			return false;

		switch (param_id) {
		/* Per-channel input controls (ch 0..7, 2 bytes) */
		case 0x04: /* INPUT_PAD_MIC */
		case 0x0c: /* INPUT_PAD_LINE */
		case 0x0e: /* INPUT_PHASE_INVERT */
		case 0x13: /* INPUT_PHANTOM */
			return ch <= 7 && w_length == 2;

		/* Ch 1..2 Hi-Z Instrument (ch 0..1, 2 bytes) */
		case 0x0d: /* INPUT_INST */
			return ch <= 1 && w_length == 2;

		/* Read-only jack sense registers */
		case 0x14: /* INPUT_FRONT_JACK_SENSE */
			return is_in && (ch == 0xff || ch <= 1) && w_length == 2;
		case 0x15: /* INPUT_JACK_SENSE */
			return is_in && ch <= 9 && w_length == 2;

		/* Global monitor & device preference registers (ch 0xff, 2 bytes) */
		case 0x0f: /* SPEAKERS_SOURCE */
		case 0x10: /* PHONES_SOURCE */
		case 0x11: /* SPEAKERS_MONO */
		case 0x12: /* PHONES_MONO */
		case 0x55: /* MIXER_OUT_LEVEL */
		case 0x66: /* DEVICE_MODE */
		case 0x70: /* WORDCLOCK_IMPEDANCE */
		case 0x71: /* WORDCLOCK_THRU */
		case 0x82: /* LOOPBACK_SOURCE */
		case 0x84: /* ADAT_ON */
			return ch == 0xff && w_length == 2;

		/* SMUX mode uses ch == 0x00 on AudioFuse 8Pre hardware */
		case 0x83: /* SMUX_MODE */
			return ch == 0x00 && w_length == 2;

		/* 24-channel cue mixer Q8.8 Left/Right gains (ch 0..23, 2 bytes) */
		case 0x20: /* MIXER_CH_GAIN_L */
		case 0x38: /* MIXER_CH_GAIN_R */
			return ch <= 23 && w_length == 2;

		/* Cue mixer Analog+ADAT Mute/Solo/Show (ch 0..15, 2 bytes) */
		case 0x50: /* MIXER_MUTE_ANALOG_ADAT */
		case 0x52: /* MIXER_SOLO_ANALOG_ADAT */
			return ch <= 15 && w_length == 2;

		/* Cue mixer USB Mute/Solo/Show (ch 0..7, 2 bytes) */
		case 0x51: /* MIXER_MUTE_USB */
		case 0x53: /* MIXER_SOLO_USB */
			return ch <= 7 && w_length == 2;

		case 0x56: /* USB overview flags: snapshot only in the app */
			return is_in && ch <= 7 && w_length == 2;
		case 0x57: /* Analog/ADAT overview flags */
			return is_in && ch <= 15 && w_length == 2;

		/* Stereo link pairs (ch 0..11, 2 bytes) */
		case 0x58: /* MIXER_STEREO_SHOW */
			return ch <= 11 && w_length == 2;

		/* 16-byte ASCII Device Name (ch 0xff, 16 bytes) */
		case 0x68: /* DEVICE_NAME */
			return ch == 0xff && w_length == 16;

		/* 16-byte Read-only Serial Number (ch 0xff, 16 bytes, IN only) */
		case 0x72: /* SERIAL_NUMBER */
			return is_in && ch == 0xff && w_length == 16;

		/* Output pair routing (ch 0..3, 2 bytes) */
		case 0x7a: /* ANALOG_OUT_SOURCE */
		case 0x7e: /* ADAT_OUT_SOURCE */
			return ch <= 3 && w_length == 2;

		default:
			return false;
		}
	}

	case 0x28: /* UAC2 Clock Selector 40 */
		return b_request == AF8PRE_UAC2_CUR &&
		       w_value == 0x0100 &&
		       w_length == 1;

	case 0x29: /* UAC2 Internal Clock Source 41 */
		if (b_request == AF8PRE_UAC2_CUR) {
			if (w_value == 0x0100 && w_length == 4)
				return true; /* Sample rate IN or OUT */
			if (is_in && w_value == 0x0200 && w_length == 1)
				return true; /* Validity IN only */
			return false;
		}
		return false;

	case 0x2b: /* UAC2 WORD Clock Source 43 (Validity IN only) */
	case 0x2c: /* UAC2 ADAT Clock Source 44 (Validity IN only) */
		return is_in &&
		       b_request == AF8PRE_UAC2_CUR &&
		       w_value == 0x0200 &&
		       w_length == 1;

	default:
		return false;
	}
}

/*
 * Validate OUT payload values for bounded discrete controls, clock selectors,
 * sample rates, and ASCII device names.
 */
static inline bool af8pre_policy_allows_out_payload(u8 b_request,
						    u16 w_value,
						    u16 w_index,
						    u16 w_length,
						    const u8 *data)
{
	u8 entity = (w_index >> 8) & 0xff;

	if (!data)
		return false;

	if (entity == 0x28 && b_request == AF8PRE_UAC2_CUR && w_value == 0x0100 && w_length == 1)
		return data[0] >= 1 && data[0] <= 3;

	if (entity == 0x29 && b_request == AF8PRE_UAC2_CUR && w_value == 0x0100 && w_length == 4) {
		u32 hz = af8pre_le32(data);
		return hz == 44100 || hz == 48000 || hz == 88200 || hz == 96000;
	}

	if (entity == 0x0a && b_request == AF8PRE_UAC2_MEM) {
		u8 param_id = (w_value >> 8) & 0xff;

		if (param_id == 0x68 && w_length == 16) {
			/* Printable ASCII (0x20..0x7e) or NUL padding only */
			unsigned int i;
			for (i = 0; i < 16; i++) {
				u8 c = data[i];
				if (c != 0x00 && (c < 0x20 || c > 0x7e))
					return false;
			}
			return true;
		}

		if (w_length != 2)
			return false;

		{
			u16 val = af8pre_le16(data);
			switch (param_id) {
			/* Boolean 0..1 controls */
			case 0x0c: /* INPUT_PAD_LINE */
			case 0x0d: /* INPUT_INST */
			case 0x0e: /* INPUT_PHASE_INVERT */
			case 0x0f: /* SPEAKERS_SOURCE */
			case 0x11: /* SPEAKERS_MONO */
			case 0x12: /* PHONES_MONO */
			case 0x13: /* INPUT_PHANTOM */
			case 0x50: /* MIXER_MUTE_ANALOG_ADAT */
			case 0x51: /* MIXER_MUTE_USB */
			case 0x52: /* MIXER_SOLO_ANALOG_ADAT */
			case 0x53: /* MIXER_SOLO_USB */
			case 0x58: /* MIXER_STEREO_SHOW */
			case 0x66: /* DEVICE_MODE */
			case 0x70: /* WORDCLOCK_IMPEDANCE */
			case 0x71: /* WORDCLOCK_THRU */
			case 0x83: /* SMUX_MODE */
			case 0x84: /* ADAT_ON */
				return val <= 1;

			/* 3-state enums (0..2) */
			case 0x04: /* INPUT_PAD_MIC */
			case 0x10: /* PHONES_SOURCE */
			case 0x82: /* LOOPBACK_SOURCE (0=Off, 1=Cue, 2=USB 17-18) */
				return val <= 2;

			/* 1-hot Analog Out mask (1/2/4/8) */
			case 0x7a:
				return val == 0x0001 || val == 0x0002 ||
				       val == 0x0004 || val == 0x0008;

			/* 1-hot ADAT Out mask (1/2/4) */
			case 0x7e:
				return val == 0x0001 || val == 0x0002 || val == 0x0004;

			/* Signed Q8.8 gains; channel maximum +12 dB and master maximum 0 dB. */
			case 0x20: /* MIXER_CH_GAIN_L */
			case 0x38: /* MIXER_CH_GAIN_R */
				return val <= 12 * 256 || val >= 0x8000;
			case 0x55: /* -infinity or -60.5..0 dB */
				return val == 0x8000 || val == 0 || val >= 65536 - 15488;

			default:
				return false;
			}
		}
	}

	return false;
}

#endif /* AUDIOFUSE8PRE_POLICY_H */
