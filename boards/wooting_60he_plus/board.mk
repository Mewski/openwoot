# Wooting 60HE+: what the build combines for this board. Every name is a directory:
# hal/<FAMILY>/, drivers/<driver>/, a feature directory under src/ (feature.mk), a variant
# directory of a strategy slot under src/ (strategy.mk).

FAMILY      := gd32f30x
DEVICE_DEFS := -DGD32F30X_XD
DRIVERS     := is31fl3741 spi_nor
FEATURES    := reference

STRATEGY_noise_recal    := stddev
STRATEGY_device_info    := mcu_flash_page_readonly
STRATEGY_layout_variant := mcu_option
