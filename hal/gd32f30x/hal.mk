# GigaDevice GD32F30x family (Cortex-M4F): what the build needs to know about the family.
# Included by the Makefile after the board.mk of a board that names this family; the board
# selects the device of the family with DEVICE_DEFS.

ARCH := -mcpu=cortex-m4 -mthumb -mfpu=fpv4-sp-d16 -mfloat-abi=hard

# TinyUSB binding. The full-speed device controller of the GD32F303 uses the STM32F1 binding
# of the stack; root port 0. The values match stock firmware 2.14.1.
HAL_DEFS := -DCFG_TUSB_MCU=OPT_MCU_STM32F1 \
            -DCFG_TUSB_RHPORT0_MODE=OPT_MODE_DEVICE

# Peripheral library: the files the code of this directory calls. The flash controller driver
# (fmc) is left out on purpose: the firmware never erases or programs the flash of this
# family, and without the driver a call does not link. system_gd32f30x.c in device/ is not
# compiled: system_clock.c takes its place.
VENDOR_DIR  := third_party/gd32f30x
VENDOR_INCS := -I$(VENDOR_DIR) -I$(VENDOR_DIR)/cmsis -I$(VENDOR_DIR)/device \
               -I$(VENDOR_DIR)/peripherals/include
VENDOR_SRC  := $(addprefix $(VENDOR_DIR)/peripherals/source/gd32f30x_, \
                 adc.c bkp.c dma.c gpio.c i2c.c pmu.c rcu.c spi.c timer.c)

# USB device controller driver of TinyUSB.
TUSB_PORT_SRC := $(TUSB_DIR)/portable/st/stm32_fsdev/dcd_stm32_fsdev.c
