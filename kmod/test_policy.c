/* SPDX-License-Identifier: GPL-2.0-only */
/*
 * Host-side unit tests for the kmod/audiofuse8pre_policy.h kernel request policy.
 * Compile & run via: make -C kmod test-policy
 */
#include <assert.h>
#include <stdio.h>
#include <string.h>

#include "audiofuse8pre_policy.h"

int main(void)
{
	u8 buf16[16];
	u8 le2[2];
	u8 le4[4];
	u8 pin;

	/* 1. Verified IN reads succeed even on read-only fd (file_has_write = false) */
	assert(af8pre_policy_allows_header(0xa1, 0x03, 0x0002, 0x0d00, 266, false, false));
	assert(af8pre_policy_allows_header(0xa1, 0x03, 0x0000, 0x0d00, 56, false, false));
	assert(af8pre_policy_allows_header(0xa1, 0x03, 0x0001, 0x0d00, 204, false, false));
	assert(af8pre_policy_allows_header(0xa1, 0x03, 0x0000, 0x1100, 152, false, false));
	assert(af8pre_policy_allows_header(0xa1, 0x01, 0x0100, 0x2800, 1, false, false));
	assert(af8pre_policy_allows_header(0xa1, 0x01, 0x0200, 0x2900, 1, false, false));
	assert(af8pre_policy_allows_header(0xa1, 0x01, 0x0200, 0x2b00, 1, false, false));
	assert(af8pre_policy_allows_header(0xa1, 0x01, 0x0200, 0x2c00, 1, false, false));
	assert(af8pre_policy_allows_header(0xa1, 0x01, 0x0100, 0x2900, 4, false, false));
	assert(af8pre_policy_allows_header(0xa1, 0x03, 0x1500, 0x0a00, 2, false, false));
	assert(af8pre_policy_allows_header(0xa1, 0x03, 0x72ff, 0x0a00, 16, false, false));

	/* 2. OUT transfers MUST be rejected when file_has_write == false (O_RDONLY) */
	assert(!af8pre_policy_allows_header(0x21, 0x03, 0x1300, 0x0a00, 2, false, false));
	assert(!af8pre_policy_allows_header(0x21, 0x03, 0x12ff, 0x0a00, 2, false, false));
	assert(!af8pre_policy_allows_header(0x21, 0x01, 0x0100, 0x2800, 1, false, false));
	assert(!af8pre_policy_allows_header(0x21, 0x01, 0x0100, 0x2900, 4, false, false));

	for (int entity = 0x28; entity <= 0x29; entity++) {
		u16 index = entity << 8;
		u16 size = entity == 0x28 ? 1 : 4;
		assert(!af8pre_policy_allows_header(0x21, 1, 0x0100, index, size, true, false));
		assert(!af8pre_policy_allows_header(0x21, 1, 0x0100, index, size, false, true));
		assert(af8pre_policy_allows_header(0x21, 1, 0x0100, index, size, true, true));
	}

	/* 3. Non-zero interfaces (IF 1, 2 streaming & IF 3 DFU) MUST always be rejected */
	assert(!af8pre_policy_allows_header(0xa1, 0x03, 0x0002, 0x0d01, 266, true, false));
	assert(!af8pre_policy_allows_header(0xa1, 0x03, 0x0000, 0x1102, 152, true, false));
	assert(!af8pre_policy_allows_header(0xa1, 0x03, 0x0000, 0x0003, 6, true, false));
	assert(!af8pre_policy_allows_header(0x21, 0x03, 0x1300, 0x0a03, 2, true, false));

	/* 4. Read-only entities/registers reject OUT even with write permission */
	assert(!af8pre_policy_allows_header(0x21, 0x03, 0x0002, 0x0d00, 266, true, false));
	assert(!af8pre_policy_allows_header(0x21, 0x03, 0x0000, 0x1100, 152, true, false));
	assert(!af8pre_policy_allows_header(0x21, 0x03, 0x14ff, 0x0a00, 2, true, false));
	assert(!af8pre_policy_allows_header(0x21, 0x03, 0x1500, 0x0a00, 2, true, false));
	assert(!af8pre_policy_allows_header(0x21, 0x03, 0x72ff, 0x0a00, 16, true, false));
	assert(!af8pre_policy_allows_header(0x21, 0x01, 0x0200, 0x2900, 1, true, false));
	assert(!af8pre_policy_allows_header(0x21, 0x01, 0x0200, 0x2b00, 1, true, false));
	assert(!af8pre_policy_allows_header(0x21, 0x01, 0x0200, 0x2c00, 1, true, false));

	/* 5. Unsupported entities, unknown params, bad channel indices, wrong lengths */
	assert(!af8pre_policy_allows_header(0xa1, 0x03, 0x0000, 0x1e00, 2, true, false)); /* unused 0x1e removed */
	assert(!af8pre_policy_allows_header(0x21, 0x03, 0x4800, 0x0a00, 2, true, false)); /* bogus preset param */
	assert(!af8pre_policy_allows_header(0x21, 0x03, 0x1308, 0x0a00, 2, true, false)); /* phantom ch 8 out of range */
	assert(!af8pre_policy_allows_header(0x21, 0x03, 0x0d02, 0x0a00, 2, true, false)); /* inst ch 2 out of range */
	assert(!af8pre_policy_allows_header(0x21, 0x03, 0x1300, 0x0a00, 4, true, false)); /* wrong wLength */
	assert(!af8pre_policy_allows_header(0x21, 0x03, 0x83ff, 0x0a00, 2, true, false)); /* SMUX requires ch=0x00 */
	assert(af8pre_policy_allows_header(0x21, 0x03, 0x8300, 0x0a00, 2, true, false));  /* SMUX ch=0x00 OK */
	assert(!af8pre_policy_allows_header(0xa1, 0x03, 0x0002, 0x0d00, 512, false, false)); /* wrong state length */
	assert(!af8pre_policy_allows_header(0xa1, 0x03, 0x0000, 0x1100, 78, false, false));  /* wrong VU length */
	assert(!af8pre_policy_allows_header(0x21, 0x01, 0x1300, 0x0a00, 2, true, false));    /* wrong bRequest on 0x0a */

	assert(!af8pre_policy_allows_header(0x21, 0x03, 0x5601, 0x0a00, 2, true, false));
	assert(!af8pre_policy_allows_header(0xa1, 0x02, 0x0100, 0x2900, 3, false, false));
	le2[0] = 1; le2[1] = 12;
	assert(!af8pre_policy_allows_out_payload(0x03, 0x2000, 0x0a00, 2, le2));
	assert(!af8pre_policy_allows_out_payload(0x03, 0x55ff, 0x0a00, 2, le2));

	/* 6. Verified OUT header + payload validation */
	assert(af8pre_policy_allows_header(0x21, 0x03, 0x1307, 0x0a00, 2, true, false));
	le2[0] = 1; le2[1] = 0;
	assert(af8pre_policy_allows_out_payload(0x03, 0x1307, 0x0a00, 2, le2));
	le2[0] = 2; le2[1] = 0;
	assert(!af8pre_policy_allows_out_payload(0x03, 0x1307, 0x0a00, 2, le2)); /* phantom must be 0..1 */

	/* Analog Out 1-hot mask (1, 2, 4, 8) vs invalid 3 */
	assert(af8pre_policy_allows_header(0x21, 0x03, 0x7a03, 0x0a00, 2, true, false));
	le2[0] = 0x08; le2[1] = 0;
	assert(af8pre_policy_allows_out_payload(0x03, 0x7a03, 0x0a00, 2, le2));
	le2[0] = 0x03; le2[1] = 0;
	assert(!af8pre_policy_allows_out_payload(0x03, 0x7a03, 0x0a00, 2, le2));

	/* Clock selector pin 1..3 */
	pin = 1;
	assert(af8pre_policy_allows_out_payload(0x01, 0x0100, 0x2800, 1, &pin));
	pin = 4;
	assert(!af8pre_policy_allows_out_payload(0x01, 0x0100, 0x2800, 1, &pin));

	/* Sample rate 48000 valid vs 192000 invalid */
	le4[0] = 0x80; le4[1] = 0xbb; le4[2] = 0x00; le4[3] = 0x00; /* 48000 */
	assert(af8pre_policy_allows_out_payload(0x01, 0x0100, 0x2900, 4, le4));
	le4[0] = 0x00; le4[1] = 0xee; le4[2] = 0x02; le4[3] = 0x00; /* 192000 */
	assert(!af8pre_policy_allows_out_payload(0x01, 0x0100, 0x2900, 4, le4));

	/* Device name 16-byte ASCII */
	memset(buf16, ' ', sizeof(buf16));
	memcpy(buf16, "Audiofuse 8Pre", 14);
	assert(af8pre_policy_allows_header(0x21, 0x03, 0x68ff, 0x0a00, 16, true, false));
	assert(af8pre_policy_allows_out_payload(0x03, 0x68ff, 0x0a00, 16, buf16));
	buf16[3] = 0x01; /* non-printable control char rejected */
	assert(!af8pre_policy_allows_out_payload(0x03, 0x68ff, 0x0a00, 16, buf16));

	printf("audiofuse8pre_policy: all assertions passed\n");
	return 0;
}
