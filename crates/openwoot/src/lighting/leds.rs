#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Led(pub u8);

impl Led {
    #[must_use]
    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Key {
    pub row: u8,
    pub column: u8,
}

impl Key {
    #[must_use]
    pub const fn new(row: u8, column: u8) -> Self {
        Self { row, column }
    }

    #[must_use]
    pub const fn row(self) -> usize {
        self.row as usize
    }

    #[must_use]
    pub const fn column(self) -> usize {
        self.column as usize
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LedPosition {
    pub page: u8,
    pub offset: u8,
}

impl LedPosition {
    #[must_use]
    pub const fn new(page: u8, offset: u8) -> Self {
        Self { page, offset }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FramePage {
    pub bank: u8,
    pub first_register: u8,
    pub length: u8,
}

impl FramePage {
    #[must_use]
    pub const fn new(bank: u8, first_register: u8, length: u8) -> Self {
        Self {
            bank,
            first_register,
            length,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Gains {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

impl Gains {
    pub const FULL: Self = Self {
        red: 255,
        green: 255,
        blue: 255,
    };
}

#[derive(Debug)]
pub struct LedMap<const PAGES: usize> {
    pub positions: &'static [Option<LedPosition>],
    pub pages: [FramePage; PAGES],
    pub gains: Gains,
}

impl<const PAGES: usize> LedMap<PAGES> {
    #[must_use]
    pub fn position(&self, led: Led) -> Option<LedPosition> {
        self.positions.get(led.index()).copied().flatten()
    }

    #[must_use]
    pub fn page(&self, page: usize) -> Option<FramePage> {
        self.pages.get(page).copied()
    }

    #[must_use]
    pub fn fits(&self, page_length: usize) -> bool {
        self.pages
            .iter()
            .all(|page| usize::from(page.length) <= page_length)
            && self.positions.iter().flatten().all(|position| {
                self.page(usize::from(position.page)).is_some_and(|page| {
                    usize::from(position.offset) + 3 <= usize::from(page.length)
                })
            })
    }
}

#[derive(Debug)]
pub struct Variant<const ROWS: usize, const COLUMNS: usize> {
    pub leds: [[Option<Led>; COLUMNS]; ROWS],
    pub spacebar: Key,
    pub spacebar_fill: &'static [u8],
}

impl<const ROWS: usize, const COLUMNS: usize> Variant<ROWS, COLUMNS> {
    #[must_use]
    pub fn led(&self, row: usize, column: usize) -> Option<Led> {
        self.leds.get(row)?.get(column).copied().flatten()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Geometry {
    pub first_key_row: u8,
    pub level_meter_segments: u8,
}
