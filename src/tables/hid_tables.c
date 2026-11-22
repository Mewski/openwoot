/* Constant tables of module `hid` (definitions). */
#include "hid_tables.h"

/* Gamepad detection pattern per XInput player index p = 0..3: [p] left X, [4 + p] left Y, [8 + p]
 * right stick; stick deflections as a fraction of full scale (-1.0 .. 1.0) */
const float k_hid_xinput_detection_stick[12] = {
     1.0f,  1.0f, -1.0f, -1.0f,
     1.0f, -1.0f, -1.0f,  1.0f,
     0.0f,  0.0f,  0.0f,  0.0f,
};
