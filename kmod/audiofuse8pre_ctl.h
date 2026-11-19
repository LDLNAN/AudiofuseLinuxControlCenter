/* SPDX-License-Identifier: GPL-2.0-only */
#ifndef AUDIOFUSE8PRE_CTL_H
#define AUDIOFUSE8PRE_CTL_H

#ifdef __KERNEL__
#include <linux/ioctl.h>
#include <linux/types.h>
#else
#include <linux/ioctl.h>
#include <linux/types.h>
#endif

#define AF8PRE_USB_VID 0x1c75
#define AF8PRE_USB_PID 0xaf0e

#define AF8PRE_MAX_DATA_LEN 512

#define AF8PRE_CTL_MAGIC 'A'

struct af8pre_device_info {
	__u16 vid;
	__u16 pid;
	__u16 bcd_device;
	__u8 bus_num;
	__u8 dev_num;
};

struct af8pre_ctrl_transfer {
	__u8 bm_request_type; /* 0xa1 (Class Interface IN) or 0x21 (Class Interface OUT) */
	__u8 b_request;       /* 0x03 (MEM), 0x01 (CUR), or 0x02 (RANGE) */
	__u16 w_value;
	__u16 w_index;        /* low byte MUST be 0x00 (Interface 0 only) */
	__u16 w_length;       /* <= AF8PRE_MAX_DATA_LEN */
	__u32 timeout_ms;     /* clamped to 10..5000 */
	__s32 transferred;    /* output: bytes transferred */
	__u8 bus_num;         /* optional bus filter (0 = first matching device) */
	__u8 dev_num;         /* optional dev filter (0 = first matching device) */
	__u16 reserved;
	__u8 data[AF8PRE_MAX_DATA_LEN];
};

#define AF8PRE_IOCTL_DEVICE_INFO   _IOWR(AF8PRE_CTL_MAGIC, 1, struct af8pre_device_info)
#define AF8PRE_IOCTL_CTRL_TRANSFER _IOWR(AF8PRE_CTL_MAGIC, 2, struct af8pre_ctrl_transfer)

#endif /* AUDIOFUSE8PRE_CTL_H */
