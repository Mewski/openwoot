/* Public function of the effect layer dispatcher (rgb_effect_layers.c). */
#ifndef RGB_RGB_EFFECT_LAYERS_H_
#define RGB_RGB_EFFECT_LAYERS_H_

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#include "rgb/rgb_types.h"
#include "rgb/rgb_data.h"

void rgb_render_effect_layers(rgb_frame_t *frame, uint32_t elapsed_ms);

#endif /* RGB_RGB_EFFECT_LAYERS_H_ */
