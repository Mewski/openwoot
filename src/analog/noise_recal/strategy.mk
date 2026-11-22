# Strategy slot noise_recal: how the idle noise of a key is measured and turned into its rest
# level and noise band. A board selects one variant with STRATEGY_noise_recal.
#
# Interface: noise_recal.h in this directory. Every sub-directory is a variant with the files
# noise_stats.h (analog_noise_stats_t) and noise_recal.c (analog_noise_stats_restart,
# analog_noise_stats_add_sample, analog_apply_noise_recalibration).
