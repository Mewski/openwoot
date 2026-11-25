use gpui_kit::{
    App, BorderStyle, Bounds, Hsla, IntoElement, Pixels, Point, RenderOnce, Size, Styled as _,
    Window, canvas, fill, hsla, outline, point, px, quad, size,
};
use openwoot_protocol::lighting::Rgb;

use super::screen::{Screen, Texel};
use crate::color_picker::hsla_of;

const CELLS: u8 = 11;
const CELL: f32 = 10.0;
const FRAME: f32 = 4.0;
const RADIUS: f32 = 12.0;
const SIDE: f32 = CELL * CELLS as f32 + 2.0 * FRAME;
const GRID_LINE: f32 = 1.0;
const WHITE: Hsla = hsla(0.0, 0.0, 1.0, 1.0);
const BLACK: Hsla = hsla(0.0, 0.0, 0.0, 1.0);
const GRID: Hsla = hsla(0.0, 0.0, 0.0, 0.12);
const RIM: Hsla = hsla(0.0, 0.0, 0.0, 0.35);

#[derive(IntoElement)]
pub(super) struct Magnifier {
    pixels: Vec<Option<Rgb>>,
    origin: Point<Pixels>,
}

impl Magnifier {
    pub(super) fn new(screen: &Screen, pointer: Point<Pixels>, viewport: Size<Pixels>) -> Self {
        let centre = screen.texel(pointer, viewport);
        Self {
            pixels: neighbourhood(screen, centre),
            origin: pointer - point(px(SIDE / 2.0), px(SIDE / 2.0)),
        }
    }
}

impl RenderOnce for Magnifier {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let pixels = self.pixels;
        canvas(
            |_, _, _| {},
            move |bounds, (), window, _| paint(bounds, &pixels, window),
        )
        .absolute()
        .left(self.origin.x)
        .top(self.origin.y)
        .size(px(SIDE))
    }
}

fn neighbourhood(screen: &Screen, centre: Texel) -> Vec<Option<Rgb>> {
    let reach = i32::from(CELLS / 2);
    (-reach..=reach)
        .flat_map(|rows| (-reach..=reach).map(move |columns| (columns, rows)))
        .map(|(columns, rows)| screen.color_beside(centre, columns, rows))
        .collect()
}

fn paint(bounds: Bounds<Pixels>, pixels: &[Option<Rgb>], window: &mut Window) {
    window.paint_quad(quad(
        bounds,
        px(RADIUS),
        WHITE,
        px(GRID_LINE),
        RIM,
        BorderStyle::Solid,
    ));
    let grid = Bounds::new(
        bounds.origin + point(px(FRAME), px(FRAME)),
        size(px(SIDE - 2.0 * FRAME), px(SIDE - 2.0 * FRAME)),
    );
    for (row, line) in (0_u8..).zip(pixels.chunks(usize::from(CELLS))) {
        for (column, pixel) in (0_u8..).zip(line) {
            window.paint_quad(fill(cell(grid, column, row), pixel.map_or(BLACK, hsla_of)));
        }
    }
    for line in 1..CELLS {
        let offset = px(f32::from(line) * CELL);
        window.paint_quad(fill(
            Bounds::new(
                grid.origin + point(offset, px(0.0)),
                size(px(GRID_LINE), grid.size.height),
            ),
            GRID,
        ));
        window.paint_quad(fill(
            Bounds::new(
                grid.origin + point(px(0.0), offset),
                size(grid.size.width, px(GRID_LINE)),
            ),
            GRID,
        ));
    }
    let centre = cell(grid, CELLS / 2, CELLS / 2);
    window.paint_quad(outline(centre.dilate(px(1.0)), WHITE, BorderStyle::Solid));
    window.paint_quad(outline(centre, BLACK, BorderStyle::Solid));
}

fn cell(grid: Bounds<Pixels>, column: u8, row: u8) -> Bounds<Pixels> {
    Bounds::new(
        grid.origin + point(px(f32::from(column) * CELL), px(f32::from(row) * CELL)),
        size(px(CELL), px(CELL)),
    )
}
