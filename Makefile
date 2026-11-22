# openwoot: open firmware for Wooting hall-effect keyboards.
#
#   make boards                   list the boards (the directories under boards/)
#   make BOARD=<board>            build the firmware of one board into build/<board>/
#   make BOARD=<board> LTO=1      link-time optimisation: smaller image, see docs/building.md
#   make BOARD=<board> size       section sizes of the ELF
#   make BOARD=<board> flash      send the keyboard into update mode and write the image
#   make BOARD=<board> clean
#   make all-boards               build every board
#   make check                    check the rules of the tree, and the registration lists of
#                                 every board that links (tools/check_tree.py)
#
# One layer of a board can be compiled without linking: make BOARD=<board> hal-objects,
# driver-objects, board-objects, src-objects or third-party-objects.
#
# The compiler is taken from PATH. Set TOOLCHAIN to the bin/ directory of another one,
# with a trailing slash: make TOOLCHAIN=/opt/arm-gnu-toolchain/bin/
#
# This file names no board, microcontroller family, driver, feature or strategy. A board is a
# directory boards/<board>/ whose board.mk lists what the firmware of the board consists of;
# everything a board.mk may list is found by its directory:
#
#   family     hal/<family>/hal.mk
#   driver     drivers/<driver>/driver.mk, which sets DRIVER_CLASS
#   feature    src/<module>/<feature>/feature.mk, which may set FEATURE_REQUIRES
#   strategy   src/<module>/<slot>/strategy.mk marks a slot; every sub-directory is a variant

LTO        ?= 0
OPT        ?= -O2
BUILD_ROOT ?= build

TOOLCHAIN ?=
CC      := $(TOOLCHAIN)arm-none-eabi-gcc
OBJCOPY := $(TOOLCHAIN)arm-none-eabi-objcopy
SIZE    := $(TOOLCHAIN)arm-none-eabi-size
PYTHON  ?= python3

BOARDS := $(sort $(patsubst boards/%/board.mk,%,$(wildcard boards/*/board.mk)))

# A board whose board.json says "build": "bring-up" does not link yet. Its default goal compiles
# every layer for it, so that a change that breaks the board is seen, but links no image.
BRING_UP_ENTRY := '"build"[[:space:]]*:[[:space:]]*"bring-up"'

.DEFAULT_GOAL := all

# ---- goals that need no board ------------------------------------------------------------

BOARDLESS_GOALS := boards all-boards check

.PHONY: $(BOARDLESS_GOALS)
boards:
	@printf '%s\n' $(BOARDS)

all-boards:
	@set -e; for board in $(BOARDS); do \
	    $(MAKE) --no-print-directory BOARD=$$board BUILD=$(BUILD_ROOT)/$$board; \
	done

# The tree rules, then for every board that links: both images (LTO=0 and LTO=1), built into a
# temporary directory that is removed afterwards, and the check of their registration lists.
LINKING_BOARDS = $(sort $(patsubst boards/%/board.json,%,\
                   $(shell grep -L $(BRING_UP_ENTRY) $(BOARDS:%=boards/%/board.json))))

check:
	$(PYTHON) tools/check_tree.py
	@set -e; scratch=$$(mktemp -d); trap 'rm -rf "$$scratch"' EXIT; \
	for board in $(LINKING_BOARDS); do \
	    for lto in 0 1; do \
	        $(MAKE) -s --no-print-directory BOARD=$$board LTO=$$lto BUILD=$$scratch/$$board-lto$$lto \
	            $$scratch/$$board-lto$$lto/openwoot-$$board.elf; \
	    done; \
	    $(PYTHON) tools/check_tree.py --elf $$scratch/$$board-lto0/openwoot-$$board.elf \
	        $$scratch/$$board-lto1/openwoot-$$board.elf; \
	done

ifeq ($(strip $(BOARD)),)

ifneq ($(if $(MAKECMDGOALS),$(filter-out $(BOARDLESS_GOALS),$(MAKECMDGOALS)),all),)
$(error usage: make BOARD=<board>; boards: $(BOARDS))
endif

else

# ---- the board and what it lists -----------------------------------------------------------

ifneq ($(words $(BOARD))$(filter $(BOARD),$(BOARDS)),1$(BOARD))
$(error unknown board '$(BOARD)'; boards: $(BOARDS))
endif

BOARD_DIR := boards/$(BOARD)
BUILD     ?= $(BUILD_ROOT)/$(BOARD)
TARGET    := $(BUILD)/openwoot-$(BOARD)

# board.mk sets FAMILY, DEVICE_DEFS (the device define of the family's library), DRIVERS,
# FEATURES and one STRATEGY_<slot> per strategy slot.
FAMILY      :=
DEVICE_DEFS :=
DRIVERS     :=
FEATURES    :=
include $(BOARD_DIR)/board.mk

# Family.
FAMILIES := $(sort $(patsubst hal/%/hal.mk,%,$(wildcard hal/*/hal.mk)))
ifneq ($(words $(FAMILY))$(filter $(FAMILY),$(FAMILIES)),1$(FAMILY))
$(error board '$(BOARD)': unknown family '$(FAMILY)'; families: $(FAMILIES))
endif
HAL_DIR := hal/$(FAMILY)

# Features.
FEATURE_DIRS := $(sort $(patsubst %/feature.mk,%,$(wildcard src/*/*/feature.mk)))
ALL_FEATURES := $(sort $(notdir $(FEATURE_DIRS)))
define load_feature
FEATURE_REQUIRES :=
include $(1)/feature.mk
FEATURE_DIR_$(notdir $(1)) := $(1)
FEATURE_REQUIRES_$(notdir $(1)) := $$(FEATURE_REQUIRES)
endef
$(foreach dir,$(FEATURE_DIRS),$(eval $(call load_feature,$(dir))))

ifneq ($(filter-out $(ALL_FEATURES),$(FEATURES)),)
$(error board '$(BOARD)': unknown feature '$(firstword $(filter-out $(ALL_FEATURES),$(FEATURES)))'; features: $(ALL_FEATURES))
endif
$(foreach feature,$(FEATURES),$(foreach required,$(FEATURE_REQUIRES_$(feature)),\
  $(if $(filter $(required),$(FEATURES)),,\
    $(error board '$(BOARD)': feature '$(feature)' requires '$(required)'))))

# Strategies: one variant per slot.
SLOT_DIRS := $(sort $(patsubst %/strategy.mk,%,$(wildcard src/*/*/strategy.mk)))
variants = $(sort $(notdir $(patsubst %/.,%,$(wildcard $(1)/*/.))))
STRATEGY_DIRS :=
define select_variant
ifneq ($$(words $$(STRATEGY_$(notdir $(1))))$$(filter $$(STRATEGY_$(notdir $(1))),$(call variants,$(1))),1$$(STRATEGY_$(notdir $(1))))
$$(error board '$(BOARD)': STRATEGY_$(notdir $(1)) must be one of: $(call variants,$(1)))
endif
STRATEGY_DIRS += $(1)/$$(STRATEGY_$(notdir $(1)))
endef
$(foreach dir,$(SLOT_DIRS),$(eval $(call select_variant,$(dir))))

# Drivers: exactly one of every class.
ALL_DRIVERS := $(sort $(patsubst drivers/%/driver.mk,%,$(wildcard drivers/*/driver.mk)))
define load_driver
DRIVER_CLASS :=
include drivers/$(1)/driver.mk
DRIVER_CLASS_$(1) := $$(DRIVER_CLASS)
endef
$(foreach driver,$(ALL_DRIVERS),$(eval $(call load_driver,$(driver))))
DRIVER_CLASSES := $(sort $(foreach driver,$(ALL_DRIVERS),$(DRIVER_CLASS_$(driver))))
drivers_of_class = $(strip $(foreach driver,$(2),$(if $(filter $(1),$(DRIVER_CLASS_$(driver))),$(driver))))

ifneq ($(filter-out $(ALL_DRIVERS),$(DRIVERS)),)
$(error board '$(BOARD)': unknown driver '$(firstword $(filter-out $(ALL_DRIVERS),$(DRIVERS)))'; drivers: $(ALL_DRIVERS))
endif
$(foreach class,$(DRIVER_CLASSES),\
  $(if $(call drivers_of_class,$(class),$(DRIVERS)),,\
    $(error board '$(BOARD)': no driver for class '$(class)'; drivers of that class: $(call drivers_of_class,$(class),$(ALL_DRIVERS))))\
  $(if $(word 2,$(call drivers_of_class,$(class),$(DRIVERS))),\
    $(error board '$(BOARD)': more than one driver for class '$(class)': $(call drivers_of_class,$(class),$(DRIVERS)))))

# 1 if the board is in bring-up (BRING_UP_ENTRY above).
BRING_UP := $(shell grep -c $(BRING_UP_ENTRY) $(BOARD_DIR)/board.json 2>/dev/null)

# ---- the family --------------------------------------------------------------------------

TUSB_DIR := third_party/tinyusb/src

# hal.mk sets ARCH (processor options), HAL_DEFS (the TinyUSB binding), VENDOR_INCS and
# VENDOR_SRC (the peripheral library of the chip maker: the files the family code calls) and
# TUSB_PORT_SRC (the USB device controller driver of TinyUSB).
ARCH          :=
HAL_DEFS      :=
VENDOR_INCS   :=
VENDOR_SRC    :=
TUSB_PORT_SRC :=
include $(HAL_DIR)/hal.mk

ifeq ($(strip $(ARCH)),)
$(error $(HAL_DIR)/hal.mk does not set ARCH)
endif
ifeq ($(strip $(HAL_DEFS)),)
$(error $(HAL_DIR)/hal.mk does not set HAL_DEFS)
endif
ifeq ($(strip $(VENDOR_INCS)),)
$(error $(HAL_DIR)/hal.mk does not set VENDOR_INCS)
endif
ifeq ($(strip $(TUSB_PORT_SRC)),)
$(error $(HAL_DIR)/hal.mk does not set TUSB_PORT_SRC)
endif

# ---- sources -----------------------------------------------------------------------------

SRC_SRC    := $(wildcard src/*/*.c) \
              $(foreach feature,$(FEATURES),$(wildcard $(FEATURE_DIR_$(feature))/*.c)) \
              $(foreach dir,$(STRATEGY_DIRS),$(wildcard $(dir)/*.c)) \
              $(wildcard $(BOARD_DIR)/defaults/*.c)
DRIVER_SRC := $(foreach driver,$(DRIVERS),$(wildcard drivers/$(driver)/*.c))
HAL_SRC    := $(wildcard $(HAL_DIR)/*.c)
TABLE_SRC  := $(wildcard $(BOARD_DIR)/tables/*.c)

# The link order of the firmware is the sort order of the source paths.
FW_SRC := $(sort $(SRC_SRC) $(DRIVER_SRC) $(HAL_SRC) $(TABLE_SRC))

LFS_SRC := third_party/littlefs/lfs.c third_party/littlefs/lfs_util.c
PB_SRC  := third_party/nanopb/pb_common.c third_party/nanopb/pb_encode.c third_party/nanopb/pb_decode.c

# TinyUSB: the stack without the drivers in portable/, plus the driver of the family.
TUSB_SRC := $(sort $(filter-out $(TUSB_DIR)/portable/%,$(wildcard $(TUSB_DIR)/*.c $(TUSB_DIR)/*/*.c \
                       $(TUSB_DIR)/*/*/*.c $(TUSB_DIR)/*/*/*/*.c)) $(TUSB_PORT_SRC))

THIRD_PARTY_SRC := $(LFS_SRC) $(PB_SRC) $(VENDOR_SRC) $(TUSB_SRC)

objects = $(patsubst %.c,$(BUILD)/%.o,$(1))
OBJ := $(call objects,$(FW_SRC) $(THIRD_PARTY_SRC))
DEP := $(OBJ:.o=.d)

# ---- flags -------------------------------------------------------------------------------

CFLAGS := $(ARCH) -std=gnu11 $(OPT) -g -fshort-enums -ffunction-sections -fdata-sections \
          -Wall -Wextra -Wno-unused-parameter -MMD -MP
LDFLAGS := $(ARCH) $(OPT) -T$(BOARD_DIR)/linker.ld -L$(HAL_DIR) -Wl,--gc-sections \
           -Wl,-Map=$(TARGET).map \
           --specs=nosys.specs -Wl,--start-group -lc -lm -lnosys -Wl,--end-group

ifeq ($(LTO),1)
CFLAGS  += -flto
LDFLAGS += -flto
endif

# Some firmware code relies on integer division by zero yielding 0, which is what the
# processor does while its divide-by-zero trap is off. Without this option GCC replaces a
# path on which it can prove the divisor is zero by a trap instruction.
FW_CFLAGS := -fno-isolate-erroneous-paths-dereference

# Third-party code is compiled without the project's warning set.
THIRD_PARTY_CFLAGS := -w

# -DFEATURE_<NAME>=1 for a feature the board lists, =0 for every other feature that exists.
upper = $(shell printf '%s' '$(1)' | tr '[:lower:]' '[:upper:]')
FEATURE_DEFS := $(foreach feature,$(ALL_FEATURES),\
                  -DFEATURE_$(call upper,$(feature))=$(if $(filter $(feature),$(FEATURES)),1,0))

LIB_DEFS := -DPB_NO_ERRMSG -DPB_WITHOUT_64BIT \
            -DLFS_NO_MALLOC -DLFS_NO_ASSERT -DLFS_NO_DEBUG -DLFS_NO_WARN -DLFS_NO_ERROR
LIB_INCS := -Ithird_party/littlefs -Ithird_party/nanopb

# TinyUSB and its configuration header (src/usb/tusb_config.h).
TUSB_INCS := -I$(TUSB_DIR) -Isrc/usb

# What each layer sees. The include paths enforce the layering: firmware and drivers have no
# path to the peripheral library or into the family directory (apart from its inline headers), and
# the family has no path into src/.
HAL_INCS     := -Ihal/include -I$(HAL_DIR)/inline
SRC_FLAGS    := $(FEATURE_DEFS) $(LIB_DEFS) $(HAL_DEFS) \
                -I$(BOARD_DIR) -Isrc -Isrc/proto $(STRATEGY_DIRS:%=-I%) \
                -Idrivers/include $(HAL_INCS) $(LIB_INCS) $(TUSB_INCS)
DRIVER_FLAGS := $(HAL_DEFS) -I$(BOARD_DIR) -Idrivers/include $(HAL_INCS) $(TUSB_INCS)
HAL_FLAGS    := $(DEVICE_DEFS) $(HAL_DEFS) -I$(BOARD_DIR) $(HAL_INCS) -I$(HAL_DIR) \
                $(VENDOR_INCS) $(TUSB_INCS)
TABLE_FLAGS  := -I$(BOARD_DIR) -Isrc
THIRD_PARTY_FLAGS := $(DEVICE_DEFS) $(HAL_DEFS) $(LIB_DEFS) $(LIB_INCS) $(VENDOR_INCS) $(TUSB_INCS)

# ---- rules -------------------------------------------------------------------------------

.PHONY: all image bring-up size flash clean \
        hal-objects driver-objects board-objects src-objects third-party-objects

ifeq ($(BRING_UP),1)
all: bring-up
else
all: image
endif

image: $(TARGET).elf $(TARGET).bin $(TARGET).hex size

bring-up: hal-objects driver-objects board-objects src-objects third-party-objects
	@echo "board '$(BOARD)' is in bring-up ($(BOARD_DIR)/board.json): every layer compiles; no image is linked"

hal-objects: $(call objects,$(HAL_SRC))
driver-objects: $(call objects,$(DRIVER_SRC))
board-objects: $(call objects,$(TABLE_SRC))
src-objects: $(call objects,$(SRC_SRC))
third-party-objects: $(call objects,$(THIRD_PARTY_SRC))

$(BUILD)/src/%.o $(BUILD)/$(BOARD_DIR)/defaults/%.o: LAYER_FLAGS := $(FW_CFLAGS) $(SRC_FLAGS)
$(BUILD)/drivers/%.o:              LAYER_FLAGS := $(FW_CFLAGS) $(DRIVER_FLAGS)
$(BUILD)/hal/%.o:                  LAYER_FLAGS := $(FW_CFLAGS) $(HAL_FLAGS)
$(BUILD)/$(BOARD_DIR)/tables/%.o:  LAYER_FLAGS := $(FW_CFLAGS) $(TABLE_FLAGS)
$(BUILD)/third_party/%.o:          LAYER_FLAGS := $(THIRD_PARTY_CFLAGS) $(THIRD_PARTY_FLAGS)

$(BUILD)/%.o: %.c
	@mkdir -p $(dir $@)
	$(CC) $(CFLAGS) $(LAYER_FLAGS) -c $< -o $@

$(TARGET).elf: $(OBJ) $(BOARD_DIR)/linker.ld $(HAL_DIR)/sections.ld
	$(CC) $(OBJ) $(LDFLAGS) -o $@

$(TARGET).bin: $(TARGET).elf
	$(OBJCOPY) -O binary --gap-fill 0xFF $< $@

$(TARGET).hex: $(TARGET).elf
	$(OBJCOPY) -O ihex $< $@

size: $(TARGET).elf
	$(SIZE) $<

flash: $(TARGET).hex
	$(PYTHON) tools/openwoot.py enter
	$(PYTHON) tools/openwoot.py flash --yes $<

clean:
	rm -rf $(BUILD)

-include $(DEP)

endif
