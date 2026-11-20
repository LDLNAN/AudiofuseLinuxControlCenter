// SPDX-License-Identifier: GPL-2.0-only
/*
 * audiofuse8pre_ctl - Non-intrusive EP0 control helper for Arturia AudioFuse 8Pre (1c75:af0e)
 *
 * Linux usbfs (drivers/usb/core/devio.c:check_ctrlrecip) rejects userspace
 * USB_TYPE_CLASS | USB_RECIP_INTERFACE (0xa1/0x21) control transfers targeting
 * Interface 0 with -EBUSY whenever snd-usb-audio owns Interface 0.
 *
 * This helper does NOT register a usb_driver and NEVER claims or detaches any
 * USB interface. snd-usb-audio stays bound to Interfaces 0, 1, and 2 while
 * PipeWire/ALSA audio streams uninterrupted, and Interface 3 (DFU) is strictly
 * rejected by kernel-side validation.
 */

#include <linux/fs.h>
#include <linux/kernel.h>
#include <linux/miscdevice.h>
#include <linux/module.h>
#include <linux/mutex.h>
#include <linux/slab.h>
#include <linux/uaccess.h>
#include <linux/usb.h>

#include "audiofuse8pre_ctl.h"
#include <linux/capability.h>
#include "audiofuse8pre_policy.h"

#define AF8PRE_DEVICE_NAME "audiofuse8pre-ctl"

static DEFINE_MUTEX(af8pre_ctl_lock);

struct af8pre_find_ctx {
	u8 bus_num;
	u8 dev_num;
	struct usb_device *udev;
};

static int af8pre_match_dev(struct usb_device *udev, void *data)
{
	struct af8pre_find_ctx *ctx = data;

	if (le16_to_cpu(udev->descriptor.idVendor) != AF8PRE_USB_VID ||
	    le16_to_cpu(udev->descriptor.idProduct) != AF8PRE_USB_PID)
		return 0;

	if (ctx->bus_num != 0 && udev->bus && udev->bus->busnum != ctx->bus_num)
		return 0;

	if (ctx->dev_num != 0 && udev->devnum != ctx->dev_num)
		return 0;

	ctx->udev = usb_get_dev(udev);
	return 1;
}

static struct usb_device *af8pre_find_device(u8 bus_num, u8 dev_num)
{
	struct af8pre_find_ctx ctx = {
		.bus_num = bus_num,
		.dev_num = dev_num,
		.udev = NULL,
	};

	usb_for_each_dev(&ctx, af8pre_match_dev);
	return ctx.udev;
}

static bool af8pre_request_allowed(const struct af8pre_ctrl_transfer *req, bool file_has_write)
{
	bool clock_out = req->bm_request_type == AF8PRE_REQ_OUT &&
		(req->w_index == 0x2800 || req->w_index == 0x2900);

	if (req->reserved != 0)
		return false;

	if (!af8pre_policy_allows_header(req->bm_request_type,
					 req->b_request,
					 req->w_value,
					 req->w_index,
					 req->w_length,
					 file_has_write,
					 clock_out && capable(CAP_SYS_ADMIN)))
		return false;

	if ((req->bm_request_type & USB_DIR_IN) == 0) {
		if (!af8pre_policy_allows_out_payload(req->b_request,
						      req->w_value,
						      req->w_index,
						      req->w_length,
						      req->data))
			return false;
	}

	return true;
}

static long af8pre_ioctl_device_info(unsigned long arg)
{
	struct af8pre_device_info info;
	struct usb_device *udev;

	if (copy_from_user(&info, (void __user *)arg, sizeof(info)))
		return -EFAULT;

	udev = af8pre_find_device(info.bus_num, info.dev_num);
	if (!udev)
		return -ENODEV;

	info.vid = le16_to_cpu(udev->descriptor.idVendor);
	info.pid = le16_to_cpu(udev->descriptor.idProduct);
	info.bcd_device = le16_to_cpu(udev->descriptor.bcdDevice);
	info.bus_num = udev->bus ? (u8)udev->bus->busnum : 0;
	info.dev_num = (u8)udev->devnum;

	usb_put_dev(udev);

	if (copy_to_user((void __user *)arg, &info, sizeof(info)))
		return -EFAULT;

	return 0;
}

static long af8pre_ioctl_ctrl_transfer(struct file *filp, unsigned long arg)
{
	struct af8pre_ctrl_transfer *req;
	struct usb_device *udev = NULL;
	void *kbuf = NULL;
	unsigned int pipe;
	bool file_has_write = (filp->f_mode & FMODE_WRITE) != 0;
	int timeout_ms;
	int ret;

	req = kmalloc(sizeof(*req), GFP_KERNEL);
	if (!req)
		return -ENOMEM;

	if (copy_from_user(req, (void __user *)arg, sizeof(*req))) {
		ret = -EFAULT;
		goto out_free_req;
	}

	if (!af8pre_request_allowed(req, file_has_write)) {
		ret = -EACCES;
		goto out_free_req;
	}

	timeout_ms = req->timeout_ms;
	if (timeout_ms < 10)
		timeout_ms = 500;
	else if (timeout_ms > 5000)
		timeout_ms = 5000;

	if (req->w_length > 0) {
		kbuf = kmalloc(req->w_length, GFP_KERNEL);
		if (!kbuf) {
			ret = -ENOMEM;
			goto out_free_req;
		}
		if ((req->bm_request_type & USB_DIR_IN) == 0)
			memcpy(kbuf, req->data, req->w_length);
	}

	mutex_lock(&af8pre_ctl_lock);

	udev = af8pre_find_device(req->bus_num, req->dev_num);
	if (!udev) {
		mutex_unlock(&af8pre_ctl_lock);
		ret = -ENODEV;
		goto out_free_kbuf;
	}

	if (req->bm_request_type & USB_DIR_IN)
		pipe = usb_rcvctrlpipe(udev, 0);
	else
		pipe = usb_sndctrlpipe(udev, 0);

	ret = usb_control_msg(udev, pipe,
			      req->b_request,
			      req->bm_request_type,
			      req->w_value,
			      req->w_index,
			      kbuf,
			      req->w_length,
			      timeout_ms);

	usb_put_dev(udev);
	mutex_unlock(&af8pre_ctl_lock);

	if (ret < 0)
		goto out_free_kbuf;

	req->transferred = ret;
	if ((req->bm_request_type & USB_DIR_IN) && ret > 0)
		memcpy(req->data, kbuf, min_t(int, ret, req->w_length));

	if (copy_to_user((void __user *)arg, req, sizeof(*req))) {
		ret = -EFAULT;
		goto out_free_kbuf;
	}

	ret = 0;

out_free_kbuf:
	kfree(kbuf);
out_free_req:
	kfree(req);
	return ret;
}

static long af8pre_unlocked_ioctl(struct file *filp, unsigned int cmd, unsigned long arg)
{
	switch (cmd) {
	case AF8PRE_IOCTL_DEVICE_INFO:
		return af8pre_ioctl_device_info(arg);
	case AF8PRE_IOCTL_CTRL_TRANSFER:
		return af8pre_ioctl_ctrl_transfer(filp, arg);
	default:
		return -ENOTTY;
	}
}

static const struct file_operations af8pre_fops = {
	.owner = THIS_MODULE,
	.unlocked_ioctl = af8pre_unlocked_ioctl,
	.compat_ioctl = compat_ptr_ioctl,
};

/*
 * Restrict default device node mode to 0660 (never world-writable 0666).
 * Pair with udev/70-arturia-audiofuse.rules (GROUP="audio", TAG+="uaccess").
 */
static struct miscdevice af8pre_miscdev = {
	.minor = MISC_DYNAMIC_MINOR,
	.name = AF8PRE_DEVICE_NAME,
	.fops = &af8pre_fops,
	.mode = 0660,
};

static int __init af8pre_ctl_init(void)
{
	int ret = misc_register(&af8pre_miscdev);

	if (ret) {
		pr_err("audiofuse8pre_ctl: misc_register failed: %d\n", ret);
		return ret;
	}
	pr_info("audiofuse8pre_ctl: registered /dev/%s (0660, strict EP0 IF0 bridge for 1c75:af0e)\n",
		AF8PRE_DEVICE_NAME);
	return 0;
}

static void __exit af8pre_ctl_exit(void)
{
	misc_deregister(&af8pre_miscdev);
	pr_info("audiofuse8pre_ctl: unregistered /dev/%s\n", AF8PRE_DEVICE_NAME);
}

module_init(af8pre_ctl_init);
module_exit(af8pre_ctl_exit);

MODULE_LICENSE("GPL");
MODULE_AUTHOR("AudioFuse Linux Control Center Contributors");
MODULE_DESCRIPTION("Non-intrusive strict EP0 control helper for Arturia AudioFuse 8Pre (1c75:af0e)");
MODULE_VERSION("0.2.1");
