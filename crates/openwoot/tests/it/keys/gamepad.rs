use openwoot::{
    hid::Hat,
    keys::{
        AxisPriority, Gamepad, GamepadInput, GamepadReport, combine_axis, normalize_travel,
        response_curve, shape_stick,
    },
    usb::Mode,
    xinput::Stick,
};
use openwoot_protocol::{
    gamepad::{
        Control::{
            self, Accelerator, Back, DpadDown, Guide, HatLeft, LeftStickDown, LeftStickLeft,
            LeftStickRight, LeftTrigger, RightStickRight, RightStickUp, RxPositive, Start,
        },
        CurvePoint, Normalization, Settings,
    },
    keys::Action,
};

use super::support::{KEY_A, engine, step};

type Binding = (Control, u16, bool);
type StickSample = (f32, f32, bool, f32, Option<&'static Settings>, (f32, f32));
type XinputPass = (&'static [Binding], (u16, u8, u8, [i16; 4]));
type HandOverPass = (&'static [Binding], (i16, AxisPriority));
type HidPass = (&'static [Binding], (u16, [u16; 6], [u16; 5], Hat));

const XINPUT_STICK_MIN: f32 = 1.0 / 32_767.0;
const HID_STICK_MIN: f32 = 1.0 / 16_383.0;

const AXIS_CURVE: Settings = settings(
    [(47, 23), (139, 32), (198, 148), (228, 223)],
    Normalization::Circular,
    true,
    None,
);
const CIRCULAR: Settings = settings(
    [(0, 22), (142, 103), (186, 153), (246, 218)],
    Normalization::Circular,
    false,
    None,
);
const SQUARE_BIASED: Settings = settings(
    [(0, 29), (26, 82), (33, 168), (229, 254)],
    Normalization::Square,
    false,
    Some(70),
);
const XINPUT_CURVE: Settings = settings(
    [(53, 1), (161, 150), (183, 203), (214, 251)],
    Normalization::Circular,
    true,
    None,
);
const HAND_OVER_CURVE: Settings = settings(
    [(20, 13), (21, 107), (159, 203), (219, 220)],
    Normalization::Circular,
    true,
    None,
);
const HID_CURVE: Settings = settings(
    [(3, 3), (57, 24), (152, 121), (201, 233)],
    Normalization::Square,
    true,
    None,
);

const NORMALIZED: [(u32, f32); 7] = [
    (1, 0.0),
    (993, 0.081_962_23),
    (1696, 0.323_821_66),
    (2847, 0.719_810_6),
    (3661, 0.999_858_3),
    (3663, 1.0),
    (4094, 1.0),
];

const CURVED: [(f32, f32); 9] = [
    (1.0e-30, 0.090_196_08),
    (0.415_466_6, 0.119_044_98),
    (0.508_287_4, 0.125_490_3),
    (0.508_287_3, 0.125_490_2),
    (0.834_254_15, 0.580_392_2),
    (0.834_254_2, 0.580_392_3),
    (0.881_124_2, 0.663_563_55),
    (0.965, 0.812_401_9),
    (0.995_978, 0.867_372_7),
];

const COMBINED: [(u16, u16, AxisPriority, f32, AxisPriority); 13] = [
    (
        1,
        4095,
        AxisPriority::PositiveTookOver,
        -1.0,
        AxisPriority::Negative,
    ),
    (
        2587,
        3593,
        AxisPriority::PositiveTookOver,
        -0.976_463_7,
        AxisPriority::Negative,
    ),
    (
        100,
        1954,
        AxisPriority::Negative,
        -0.412_583_7,
        AxisPriority::Negative,
    ),
    (
        4013,
        1,
        AxisPriority::PositiveTookOver,
        1.0,
        AxisPriority::Positive,
    ),
    (
        1,
        1,
        AxisPriority::Positive,
        -0.0,
        AxisPriority::NegativeTookOver,
    ),
    (
        1,
        1,
        AxisPriority::NegativeTookOver,
        -0.0,
        AxisPriority::NegativeTookOver,
    ),
    (
        4012,
        4012,
        AxisPriority::Positive,
        -1.0,
        AxisPriority::NegativeTookOver,
    ),
    (
        3365,
        4012,
        AxisPriority::PositiveTookOver,
        -1.0,
        AxisPriority::Negative,
    ),
    (2048, 4013, AxisPriority::None, -1.0, AxisPriority::Negative),
    (
        0,
        0,
        AxisPriority::NegativeTookOver,
        0.0,
        AxisPriority::None,
    ),
    (
        43,
        43,
        AxisPriority::NegativeTookOver,
        -0.0,
        AxisPriority::NegativeTookOver,
    ),
    (
        1720,
        0,
        AxisPriority::Positive,
        0.332_078_6,
        AxisPriority::Positive,
    ),
    (
        0,
        1181,
        AxisPriority::Positive,
        -0.146_641_58,
        AxisPriority::Negative,
    ),
];

const SHAPED: [StickSample; 10] = [
    (
        -0.506_001_7,
        -0.478_074_67,
        false,
        HID_STICK_MIN,
        None,
        (-0.367_803_07, -0.347_503_42),
    ),
    (
        0.999_999_94,
        -0.294_025_45,
        true,
        HID_STICK_MIN,
        None,
        (0.959_389_3, -0.282_084_88),
    ),
    (
        -0.269_980_58,
        0.591_440_3,
        true,
        XINPUT_STICK_MIN,
        Some(&CIRCULAR),
        (-0.174_199_15, 0.381_614_12),
    ),
    (
        0.398_724_4,
        0.336_773_6,
        true,
        0.022_782_44,
        Some(&CIRCULAR),
        (0.233_534_13, 0.197_249_34),
    ),
    (
        9.410_595e-5,
        -1.817_597_3e-5,
        true,
        XINPUT_STICK_MIN,
        Some(&CIRCULAR),
        (0.084_759_82, -0.016_370_825),
    ),
    (
        6.1e-5,
        -0.0,
        false,
        HID_STICK_MIN,
        Some(&CIRCULAR),
        (0.0, 0.0),
    ),
    (
        -0.804_451_35,
        -0.497_486_26,
        false,
        HID_STICK_MIN,
        Some(&SQUARE_BIASED),
        (-0.919_024_9, -0.568_340_5),
    ),
    (
        -0.097_214_7,
        0.724_874_26,
        true,
        HID_STICK_MIN,
        Some(&SQUARE_BIASED),
        (-0.119_047_455, 0.887_668_55),
    ),
    (
        0.169_093_37,
        -0.176_147_03,
        true,
        XINPUT_STICK_MIN,
        Some(&SQUARE_BIASED),
        (0.671_449_36, -0.299_767_97),
    ),
    (
        -0.562_279_3,
        -0.391_133_64,
        true,
        HID_STICK_MIN,
        Some(&SQUARE_BIASED),
        (-0.8236, -0.245_534_57),
    ),
];

const XINPUT_PASSES: [XinputPass; 9] = [
    (
        &[
            (Start, 222, true),
            (RightStickRight, 106, false),
            (LeftTrigger, 666, false),
            (RxPositive, 1401, false),
            (RightStickUp, 62, false),
        ],
        (16, 0, 0, [0, 0, 0, 0]),
    ),
    (
        &[
            (Start, 222, true),
            (RightStickRight, 106, false),
            (LeftTrigger, 1332, true),
            (RxPositive, 2802, true),
            (RightStickUp, 62, false),
        ],
        (16, 42, 0, [0, 0, 0, 0]),
    ),
    (
        &[
            (Start, 222, true),
            (RightStickRight, 185, false),
            (LeftTrigger, 1998, true),
            (RxPositive, 4019, true),
            (RightStickUp, 1131, false),
        ],
        (16, 99, 0, [0, 0, 0, 3218]),
    ),
    (
        &[
            (Start, 2605, true),
            (RightStickRight, 264, false),
            (LeftTrigger, 2664, true),
            (RxPositive, 4019, true),
            (RightStickUp, 2200, true),
        ],
        (16, 161, 0, [0, 0, 0, 15_019]),
    ),
    (
        &[
            (Start, 2605, true),
            (RightStickRight, 343, false),
            (LeftTrigger, 3330, true),
            (Guide, 571, false),
            (RxPositive, 4019, true),
            (RightStickUp, 3269, true),
        ],
        (16, 240, 0, [0, 0, 0, 30_176]),
    ),
    (
        &[
            (Start, 2605, true),
            (RightStickRight, 422, false),
            (LeftTrigger, 3996, true),
            (Guide, 1142, false),
            (RxPositive, 4019, true),
            (RightStickUp, 4095, true),
            (DpadDown, 533, false),
        ],
        (16, 252, 0, [0, 0, 0, 32_253]),
    ),
    (
        &[
            (Start, 2605, true),
            (RightStickRight, 501, false),
            (LeftTrigger, 4021, true),
            (Guide, 1713, false),
            (RxPositive, 4014, true),
            (RightStickUp, 4095, true),
            (DpadDown, 533, false),
        ],
        (16, 252, 0, [0, 0, 0, 32_253]),
    ),
    (
        &[
            (Start, 2605, true),
            (RightStickRight, 580, false),
            (LeftTrigger, 2742, true),
            (Guide, 2284, false),
            (RxPositive, 4014, true),
            (RightStickUp, 4095, true),
            (DpadDown, 533, false),
        ],
        (16, 173, 0, [0, 0, 0, 32_253]),
    ),
    (
        &[
            (Start, 0, false),
            (RightStickRight, 659, false),
            (LeftTrigger, 1463, true),
            (Guide, 2855, true),
            (RxPositive, 2821, true),
            (RightStickUp, 4095, true),
            (DpadDown, 0, false),
        ],
        (1024, 53, 0, [0, 0, 0, 32_253]),
    ),
];

const HAND_OVER_PASSES: [HandOverPass; 10] = [
    (
        &[
            (RightStickRight, 1974, true),
            (HatLeft, 1143, false),
            (LeftStickRight, 2095, true),
            (Start, 1274, false),
            (LeftStickLeft, 1420, false),
        ],
        (23_533, AxisPriority::Positive),
    ),
    (
        &[
            (RightStickRight, 2057, true),
            (HatLeft, 1270, false),
            (LeftStickRight, 2356, true),
            (Start, 1372, false),
            (LeftStickLeft, 2840, false),
        ],
        (-26_735, AxisPriority::Negative),
    ),
    (
        &[
            (RightStickRight, 2140, true),
            (HatLeft, 1397, false),
            (LeftStickRight, 2617, true),
            (Start, 1470, false),
            (LeftStickLeft, 4095, true),
        ],
        (-28_270, AxisPriority::Negative),
    ),
    (
        &[
            (RightStickRight, 2223, true),
            (HatLeft, 1524, false),
            (LeftStickRight, 2878, true),
            (Start, 1568, false),
            (LeftStickLeft, 4095, true),
        ],
        (-28_270, AxisPriority::Negative),
    ),
    (
        &[
            (RightStickRight, 2306, true),
            (HatLeft, 1651, false),
            (LeftStickRight, 3139, true),
            (Start, 1666, false),
            (LeftStickLeft, 4095, true),
        ],
        (-28_270, AxisPriority::Negative),
    ),
    (
        &[
            (RightStickRight, 2389, true),
            (HatLeft, 1778, false),
            (LeftStickRight, 3400, true),
            (Start, 1764, false),
            (LeftStickLeft, 4095, true),
        ],
        (-28_270, AxisPriority::Negative),
    ),
    (
        &[
            (RightStickRight, 2472, true),
            (HatLeft, 1905, true),
            (LeftStickRight, 3661, true),
            (Start, 1862, false),
            (LeftStickLeft, 4095, true),
        ],
        (-28_270, AxisPriority::Negative),
    ),
    (
        &[
            (RightStickRight, 2555, true),
            (HatLeft, 2032, true),
            (LeftStickRight, 3922, true),
            (Start, 1960, false),
            (LeftStickLeft, 4095, true),
        ],
        (-28_270, AxisPriority::Negative),
    ),
    (
        &[
            (RightStickRight, 2638, true),
            (HatLeft, 2159, true),
            (LeftStickRight, 4014, true),
            (Start, 2058, false),
            (LeftStickLeft, 4095, true),
        ],
        (28_269, AxisPriority::PositiveTookOver),
    ),
    (
        &[
            (RightStickRight, 2721, true),
            (HatLeft, 2286, true),
            (LeftStickRight, 4014, true),
            (Start, 2156, false),
            (LeftStickLeft, 4013, true),
        ],
        (28_269, AxisPriority::Positive),
    ),
];

const HID_PASSES: [HidPass; 14] = [
    (
        &[
            (RightStickRight, 549, false),
            (LeftStickDown, 4014, true),
            (Accelerator, 2373, false),
            (HatLeft, 167, false),
            (Back, 571, false),
            (RightStickUp, 253, false),
            (LeftStickRight, 175, false),
        ],
        (
            0,
            [16_980, 31_352, 16_383, 16_383, 17_354, 15_872],
            [0, 0, 14_993, 0, 0],
            Hat::Centered,
        ),
    ),
    (
        &[
            (RightStickRight, 1098, true),
            (LeftStickDown, 4013, true),
            (Accelerator, 2373, false),
            (HatLeft, 334, false),
            (Back, 1142, true),
            (RightStickUp, 253, false),
            (LeftStickRight, 350, false),
        ],
        (
            32,
            [17_803, 31_352, 16_383, 16_383, 18_671, 15_872],
            [0, 0, 14_993, 0, 0],
            Hat::Centered,
        ),
    ),
    (
        &[
            (RightStickRight, 1647, true),
            (LeftStickDown, 4013, true),
            (Accelerator, 1003, false),
            (HatLeft, 501, false),
            (Back, 1713, true),
            (RightStickUp, 1078, false),
            (LeftStickRight, 525, false),
        ],
        (
            32,
            [18_627, 31_352, 16_383, 16_383, 20_913, 14_177],
            [0, 0, 3800, 0, 0],
            Hat::Centered,
        ),
    ),
    (
        &[
            (RightStickRight, 2196, true),
            (LeftStickDown, 4013, true),
            (Accelerator, 165, false),
            (HatLeft, 668, false),
            (Back, 2284, true),
            (RightStickUp, 1903, true),
            (LeftStickRight, 700, false),
        ],
        (
            32,
            [19_451, 31_352, 16_383, 16_383, 23_156, 10_807],
            [0, 0, 749, 0, 0],
            Hat::Centered,
        ),
    ),
    (
        &[
            (RightStickRight, 2745, true),
            (LeftStickDown, 4013, true),
            (Accelerator, 165, false),
            (HatLeft, 835, false),
            (Back, 2855, true),
            (RightStickUp, 2728, true),
            (LeftStickRight, 875, false),
        ],
        (
            32,
            [20_275, 31_352, 16_383, 16_383, 26_937, 5985],
            [0, 0, 749, 0, 0],
            Hat::Centered,
        ),
    ),
    (
        &[
            (RightStickRight, 3294, true),
            (LeftStickDown, 4013, true),
            (Accelerator, 165, false),
            (HatLeft, 1002, false),
            (Back, 3426, true),
            (RightStickUp, 3553, true),
            (LeftStickRight, 1050, false),
        ],
        (
            32,
            [21_099, 31_352, 16_383, 16_383, 31_352, 1414],
            [0, 0, 749, 0, 0],
            Hat::Centered,
        ),
    ),
    (
        &[
            (RightStickRight, 3843, true),
            (LeftStickDown, 4013, true),
            (Start, 4022, true),
            (Accelerator, 165, false),
            (HatLeft, 1169, false),
            (Back, 3997, true),
            (RightStickUp, 4014, true),
            (LeftStickRight, 1225, false),
        ],
        (
            48,
            [21_923, 31_352, 16_383, 16_383, 31_352, 1414],
            [0, 0, 749, 0, 0],
            Hat::Centered,
        ),
    ),
    (
        &[
            (RightStickRight, 4018, true),
            (LeftStickDown, 4013, true),
            (Start, 4022, true),
            (Accelerator, 4014, true),
            (HatLeft, 1336, false),
            (Back, 4093, true),
            (RightStickUp, 4014, true),
            (LeftStickRight, 1400, false),
        ],
        (
            48,
            [22_747, 31_352, 16_383, 16_383, 31_352, 1414],
            [0, 0, 29_940, 0, 0],
            Hat::Centered,
        ),
    ),
    (
        &[
            (RightStickRight, 4044, true),
            (LeftStickDown, 4036, true),
            (Start, 4022, true),
            (Accelerator, 4014, true),
            (HatLeft, 1503, false),
            (Back, 4093, true),
            (RightStickUp, 3767, true),
            (LeftStickRight, 1575, true),
        ],
        (
            48,
            [23_571, 31_352, 16_383, 16_383, 31_352, 1414],
            [0, 0, 29_940, 0, 0],
            Hat::Centered,
        ),
    ),
    (
        &[
            (RightStickRight, 4044, true),
            (LeftStickDown, 4036, true),
            (Start, 4022, true),
            (Accelerator, 4014, true),
            (HatLeft, 1670, false),
            (Back, 4093, true),
            (RightStickUp, 3520, true),
            (LeftStickRight, 1734, true),
        ],
        (
            48,
            [24_319, 31_352, 16_383, 16_383, 31_352, 1414],
            [0, 0, 29_940, 0, 0],
            Hat::Centered,
        ),
    ),
    (
        &[
            (RightStickRight, 4044, true),
            (LeftStickDown, 4013, true),
            (Start, 4022, true),
            (Accelerator, 4014, true),
            (HatLeft, 1837, false),
            (Back, 4093, true),
            (RightStickUp, 3273, true),
            (LeftStickRight, 1734, true),
        ],
        (
            48,
            [24_319, 31_352, 16_383, 16_383, 31_352, 1414],
            [0, 0, 29_940, 0, 0],
            Hat::Centered,
        ),
    ),
    (
        &[
            (RightStickRight, 4044, true),
            (LeftStickDown, 4013, true),
            (Start, 1918, true),
            (Accelerator, 4014, true),
            (HatLeft, 2004, true),
            (Back, 4093, true),
            (RightStickUp, 3026, true),
            (LeftStickRight, 1734, true),
        ],
        (
            48,
            [24_319, 31_352, 16_383, 16_383, 31_352, 3260],
            [0, 0, 29_940, 0, 0],
            Hat::Left,
        ),
    ),
    (
        &[
            (RightStickRight, 4013, true),
            (LeftStickDown, 4013, true),
            (Start, 1918, true),
            (Accelerator, 4014, true),
            (HatLeft, 2171, true),
            (Back, 4019, true),
            (RightStickUp, 2779, true),
            (LeftStickRight, 1734, true),
        ],
        (
            48,
            [24_319, 31_352, 16_383, 16_383, 31_352, 5518],
            [0, 0, 29_940, 0, 0],
            Hat::Left,
        ),
    ),
    (
        &[
            (RightStickRight, 4013, true),
            (LeftStickDown, 168, false),
            (Start, 1918, true),
            (Accelerator, 4013, true),
            (HatLeft, 2338, true),
            (Back, 4019, true),
            (RightStickUp, 2532, true),
            (LeftStickRight, 1734, true),
        ],
        (
            48,
            [21_269, 16_730, 16_383, 16_383, 31_352, 7777],
            [0, 0, 29_940, 0, 0],
            Hat::Left,
        ),
    ),
];

const fn settings(
    points: [(u8, u8); 4],
    normalization: Normalization,
    single_direction_axes: bool,
    left_stick_up_bias: Option<u8>,
) -> Settings {
    let [dead_zone, knee_low, knee_high, saturation] = points;
    Settings {
        curve: [
            point(dead_zone),
            point(knee_low),
            point(knee_high),
            point(saturation),
        ],
        normalization,
        single_direction_axes,
        left_stick_up_bias,
    }
}

const fn point((travel, output): (u8, u8)) -> CurvePoint {
    CurvePoint { travel, output }
}

fn assert_bits(actual: f32, expected: f32) {
    assert!(
        actual.to_bits() == expected.to_bits() || (actual.is_nan() && expected.is_nan()),
        "{actual:?} is not {expected:?}"
    );
}

fn input(mode: Mode, bindings: &[Binding]) -> GamepadInput {
    let mut input = GamepadInput::new();
    for &(control, travel, pressed) in bindings {
        input.apply(mode, control, travel, pressed);
    }
    input
}

#[test]
fn travel_is_rescaled_to_the_input_range_of_the_curve() {
    for (travel, expected) in NORMALIZED {
        assert_bits(normalize_travel(travel, Some(&AXIS_CURVE)), expected);
    }
}

#[test]
fn the_response_curve_is_linear_between_its_points() {
    for (level, expected) in CURVED {
        assert_bits(response_curve(level, Some(&AXIS_CURVE)), expected);
    }
}

#[test]
fn single_direction_axes_keep_the_deeper_or_the_later_direction() {
    for (positive, negative, before, expected, after) in COMBINED {
        let mut priority = before;
        let value = combine_axis(positive, negative, &mut priority, Some(&AXIS_CURVE));
        assert_bits(value, expected);
        assert_eq!(priority, after);
    }
}

#[test]
fn a_stick_keeps_its_direction_and_takes_the_curve_as_magnitude() {
    for (x, y, left_stick, min_magnitude, curve, (expected_x, expected_y)) in SHAPED {
        let (x, y) = shape_stick(x, y, left_stick, min_magnitude, curve);
        assert_bits(x, expected_x);
        assert_bits(y, expected_y);
    }
}

#[test]
fn the_xinput_report_carries_buttons_triggers_and_sticks() {
    let mut gamepad = Gamepad::new();
    for (bindings, expected) in XINPUT_PASSES {
        let report = gamepad.xinput_report(&input(Mode::Xinput, bindings), Some(&XINPUT_CURVE));
        let sticks = [
            report.left_stick.x,
            report.left_stick.y,
            report.right_stick.x,
            report.right_stick.y,
        ];
        assert_eq!(
            (
                report.buttons.bits(),
                report.left_trigger,
                report.right_trigger,
                sticks
            ),
            expected
        );
    }
}

#[test]
fn both_directions_bottomed_out_hand_the_axis_to_the_other_direction() {
    let mut gamepad = Gamepad::new();
    for (bindings, expected) in HAND_OVER_PASSES {
        let report = gamepad.xinput_report(&input(Mode::Xinput, bindings), Some(&HAND_OVER_CURVE));
        let [priority, ..] = gamepad.priorities();
        assert_eq!((report.left_stick.x, priority), expected);
    }
}

#[test]
fn the_hid_report_carries_buttons_axes_sliders_and_hat() {
    let mut gamepad = Gamepad::new();
    for (bindings, expected) in HID_PASSES {
        let report = gamepad.hid_report(&mut input(Mode::Gamepad, bindings), Some(&HID_CURVE));
        let axes = [
            report.x, report.y, report.z, report.rz, report.rx, report.ry,
        ];
        let sliders = [
            report.throttle,
            report.elevator,
            report.accelerator,
            report.rudder,
            report.trigger,
        ];
        assert_eq!((report.buttons, axes, sliders, report.hat), expected);
    }
}

#[test]
fn the_host_detection_sees_a_fixed_pattern() {
    let corners = [0, 1, 2, 3].map(|player| Gamepad::xinput_detection(Some(player)).left_stick);
    assert_eq!(
        corners,
        [
            Stick {
                x: 32_767,
                y: 32_767
            },
            Stick {
                x: 32_767,
                y: -32_768
            },
            Stick {
                x: -32_768,
                y: -32_768
            },
            Stick {
                x: -32_768,
                y: 32_767
            },
        ]
    );
    let unknown = Gamepad::xinput_detection(None);
    assert_eq!(unknown.left_stick, Stick::default());
    assert_eq!(
        unknown.right_stick,
        Stick {
            x: 0x7FFF,
            y: 0x7FFF
        }
    );
    assert_eq!(Gamepad::xinput_detection(Some(7)), unknown);
    let hid = Gamepad::hid_detection();
    assert_eq!(
        (hid.x, hid.y, hid.z, hid.rz, hid.rx, hid.ry, hid.hat),
        (
            0x7FFE,
            0x7FFE,
            0x3FFF,
            0x3FFF,
            0x3FFF,
            0x3FFF,
            Hat::Centered
        )
    );
}

#[test]
fn gamepad_detection_replaces_the_report_with_the_pattern_of_the_player() {
    let mut keys = engine(&[(KEY_A, Action::Gamepad(Control::ButtonA))]);
    keys.set_mode(Mode::Xinput);
    keys.set_gamepad_detection(true);
    keys.set_xinput_player(Some(1));
    step(&mut keys, &[(KEY_A, 3000)], 1);
    assert_eq!(
        keys.output().gamepad(),
        Some(&GamepadReport::Xinput(Gamepad::xinput_detection(Some(1))))
    );
    keys.set_mode(Mode::Gamepad);
    step(&mut keys, &[(KEY_A, 3000)], 1);
    assert_eq!(
        keys.output().gamepad(),
        Some(&GamepadReport::Hid(Gamepad::hid_detection()))
    );
    keys.set_gamepad_detection(false);
    step(&mut keys, &[(KEY_A, 3000)], 1);
    assert_ne!(
        keys.output().gamepad(),
        Some(&GamepadReport::Hid(Gamepad::hid_detection()))
    );
}
