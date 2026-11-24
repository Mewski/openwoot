use std::borrow::Cow;

use gpui_kit::{AssetSource, Result, SharedString};

gpui_kit::assets::icon_assets!(
    Extra,
    [
        Activity,
        ArrowUpDown,
        Blend,
        Cable,
        ChartLine,
        Circle,
        Clipboard,
        Crosshair,
        Download,
        Droplet,
        Eraser,
        FileDown,
        FileUp,
        Flame,
        Gamepad,
        Gamepad2,
        Gauge,
        Grid3x3,
        Hand,
        Joystick,
        Keyboard,
        KeyboardMusic,
        Layers,
        Layers2,
        Lightbulb,
        Link,
        ListChecks,
        Lock,
        LockOpen,
        Monitor,
        Mouse,
        MousePointer,
        Move,
        Music,
        PaintBucket,
        Paintbrush,
        Pencil,
        Pipette,
        Plus,
        Power,
        Radio,
        Rainbow,
        Repeat,
        RotateCcw,
        Save,
        ScanLine,
        SlidersHorizontal,
        Sparkles,
        SunDim,
        Thermometer,
        Timer,
        ToggleLeft,
        ToggleRight,
        Trash,
        Unlink,
        Upload,
        Usb,
        UserRound,
        Volume2,
        WandSparkles,
        Zap,
    ]
);

pub(crate) struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        match Extra.load(path)? {
            Some(bytes) => Ok(Some(bytes)),
            None => gpui_kit::assets::Assets.load(path),
        }
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut names = Extra.list(path)?;
        names.extend(gpui_kit::assets::Assets.list(path)?);
        Ok(names)
    }
}
