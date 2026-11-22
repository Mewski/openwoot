# Wooting 80HE: what the build combines for this board. Every name is a directory:
# hal/<FAMILY>/, drivers/<driver>/, a feature directory under src/ (feature.mk), a variant
# directory of a strategy slot under src/ (strategy.mk).

FAMILY      := gd32f4xx
DEVICE_DEFS := -DGD32F425
DRIVERS     := is31fl3741 spi_nor
FEATURES    := signals lightbar edition recheck

STRATEGY_noise_recal    := minmax
STRATEGY_device_info    := nor_flash_sector
STRATEGY_layout_variant := device_info
