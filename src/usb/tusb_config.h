/*
 * TinyUSB configuration shared by all boards. The values match stock firmware 2.14.1;
 * each one is followed by what it determines. The two settings that depend on the
 * microcontroller family (CFG_TUSB_MCU and the root hub port mode) are HAL_DEFS of the
 * family's hal.mk and reach this configuration as compiler options.
 *
 * Stack: third_party/tinyusb (upstream commit f452ab745e plus the local changes marked
 * "openwoot:").
 */
#ifndef TUSB_CONFIG_H_
#define TUSB_CONFIG_H_

#ifdef __cplusplus
extern "C" {
#endif

/*--------------------------------------------------------------------
 * Common
 *------------------------------------------------------------------*/

/* No RTOS: the event queue is a plain tu_fifo guarded by usbd_int_set()
 * (osal_none.h queue definition, no mutex). */
#define CFG_TUSB_OS               OPT_OS_NONE

/* No log output; TU_ASSERT is only "BKPT if a debugger is attached, then
 * return". */
#define CFG_TUSB_DEBUG            0

/* USB DMA / memory placement: default section, 4-byte alignment.
 * _hidd_itf, _usbd_ctrl_buf and the event queue buffer lie in the ordinary
 * .bss; the buffers inside hidd_interface_t start at multiples of 4
 * (+0x008, +0x808, +0x1008). */
#define CFG_TUSB_MEM_SECTION
#define CFG_TUSB_MEM_ALIGN        __attribute__ ((aligned(4)))

/*--------------------------------------------------------------------
 * Device
 *------------------------------------------------------------------*/

/* Control endpoint size: _usbd_ctrl_buf is 64 bytes, _data_stage_xact() limits
 * a packet to 64 bytes, device descriptor bMaxPacketSize0 = 0x40. */
#define CFG_TUD_ENDPOINT0_SIZE    64

/* Depth of the device event queue: 16 events of 12 bytes. This is also the
 * upstream default. */
#define CFG_TUD_TASK_QUEUE_SZ     16

/*------------- Class drivers -------------*/

/* Six HID instances (hidd_reset clears 6 * 0x104C = 0x61C8 bytes). Five are
 * used by the configuration descriptors (four when interface 0 is XInput). */
#define CFG_TUD_HID               6

/* IN and OUT buffer of each HID instance: 2048 bytes.
 * sizeof(hidd_interface_t) = 0x104C = 8 (header) + 2048 (epin_buf, +0x008)
 * + 2048 (epout_buf, +0x808) + 64 (feature report buffer, +0x1008)
 * + 4 (hid_descriptor, +0x1048); OUT transfers are armed with length 0x800
 * (hidd_open), tud_hid_n_report limits to 0x7FF / 0x800.
 * The largest report of the protocol interface is 2046 bytes + report id. */
#define CFG_TUD_HID_EP_BUFSIZE    2048

/* No other built-in class driver: _usbd_driver[] has exactly one entry, the
 * HID driver. The XInput interface is an application class driver returned by
 * usbd_app_driver_get_cb() (k_usb_app_class_driver). */
#define CFG_TUD_CDC               0
#define CFG_TUD_MSC               0
#define CFG_TUD_MIDI              0
#define CFG_TUD_AUDIO             0
#define CFG_TUD_VIDEO             0
#define CFG_TUD_VENDOR            0
#define CFG_TUD_USBTMC            0
#define CFG_TUD_DFU_RUNTIME       0
#define CFG_TUD_DFU               0
#define CFG_TUD_ECM_RNDIS         0
#define CFG_TUD_NCM               0
#define CFG_TUD_BTH               0

#ifdef __cplusplus
}
#endif

#endif /* TUSB_CONFIG_H_ */
