# GigaDevice GD32F4xx family (Cortex-M4F): what the build needs to know about the family.
# Included by the Makefile after the board.mk of a board that names this family; the board
# selects the device of the family with DEVICE_DEFS.

ARCH := -mcpu=cortex-m4 -mthumb -mfpu=fpv4-sp-d16 -mfloat-abi=hard

# TinyUSB binding. The GD32F425 uses the STM32F4 DWC2 binding of the stack; root port 1 is the
# high-speed controller. The values match stock firmware 2.14.1.
HAL_DEFS := -DCFG_TUSB_MCU=OPT_MCU_STM32F4 \
            '-DCFG_TUSB_RHPORT1_MODE=(OPT_MODE_DEVICE | OPT_MODE_HIGH_SPEED)'

# Peripheral library: the files the code of this directory calls. system_gd32f4xx.c in
# device/ is not compiled: system_clock.c takes its place.
VENDOR_DIR  := third_party/gd32f4xx
VENDOR_INCS := -I$(VENDOR_DIR) -I$(VENDOR_DIR)/cmsis -I$(VENDOR_DIR)/device \
               -I$(VENDOR_DIR)/peripherals/include
VENDOR_SRC  := $(addprefix $(VENDOR_DIR)/peripherals/source/gd32f4xx_, \
                 adc.c dma.c fmc.c gpio.c i2c.c pmu.c rcu.c spi.c timer.c)

# USB device controller driver of TinyUSB.
TUSB_PORT_SRC := $(TUSB_DIR)/portable/synopsys/dwc2/dcd_dwc2.c
