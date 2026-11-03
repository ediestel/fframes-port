//! Measured layout: text, boxes and gaps stacked in rows and columns, measured against the
//! real fonts before anything is drawn, so a band is as high as its caption and a highlight
//! lands on the words it marks.
//!
//! A [`Block`] describes the content. [`Block::measure`] resolves it against the fonts into a
//! [`Measured`] tree that keeps every child's size, the wrapped lines and the font metrics.
//! [`Measured::render`] draws that tree with its top left corner at a position, and
//! [`Measured::place`] lists the rectangles of the named blocks, for decorations drawn around
//! them. The width a text wraps at is given by the author (`max_width`); nothing is inferred
//! from the parent, so measuring is one pass and the result is cached by the text cache,
//! independent of where the block ends up.
//!
//! ```no_run
//! use fframes::layout::{Block, FrameMeasurer, TextBlock};
//!
//! let caption = Block::Text(
//!     TextBlock::new(text, font).wrap(1560).line_height(1.3).align(fframes::TextAlign::Center).fill("#fff"),
//! );
//! let measured = caption.measure(&mut FrameMeasurer { frame: &mut frame, ctx })?;
//! let height = measured.size().height + 2.0 * padding;
//! svgr!(<g><rect y={bottom - height} height={height} /> {measured.render(x, bottom - height + padding)}</g>)
//! ```

use crate::{
    BreakLinesOpts, FFramesContext, FontMetrics, FontQuery, FontStyle, Frame, Svgr, TextAlign,
    WrappedTextStructure, svgr,
};

/// A width and a height in pixels.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

/// A rectangle in the video's pixels.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub fn right(&self) -> f32 {
        self.x + self.width
    }

    pub fn bottom(&self) -> f32 {
        self.y + self.height
    }

    pub fn center_x(&self) -> f32 {
        self.x + self.width / 2.0
    }

    pub fn center_y(&self) -> f32 {
        self.y + self.height / 2.0
    }
}

/// The direction children of a stack follow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Direction {
    #[default]
    Column,
    Row,
}

/// Where children sit across a stack's direction: the left, centre or right of a column,
/// the top, middle or bottom of a row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Align {
    #[default]
    Start,
    Center,
    End,
}

/// A piece of text in one font.
#[derive(Debug, Clone)]
pub struct TextBlock<'a> {
    pub text: String,
    pub font: FontQuery<'a>,
    /// Lines wrap at this width in pixels and the block is this wide; `None` keeps the text
    /// on one line as wide as the text.
    pub max_width: Option<usize>,
    /// Distance between baselines as a factor of the font size, like CSS `line-height`.
    pub line_height: f32,
    pub align: TextAlign,
    pub fill: &'a str,
}

impl<'a> TextBlock<'a> {
    /// One line of `text` in `font`, left aligned, black, line height 1.2.
    pub fn new(text: impl Into<String>, font: FontQuery<'a>) -> Self {
        Self {
            text: text.into(),
            font,
            max_width: None,
            line_height: 1.2,
            align: TextAlign::Left,
            fill: "#000000",
        }
    }

    pub fn wrap(mut self, max_width: usize) -> Self {
        self.max_width = Some(max_width);
        self
    }

    pub fn line_height(mut self, factor: f32) -> Self {
        self.line_height = factor;
        self
    }

    pub fn align(mut self, align: TextAlign) -> Self {
        self.align = align;
        self
    }

    pub fn fill(mut self, fill: &'a str) -> Self {
        self.fill = fill;
        self
    }
}

/// What is laid out.
pub enum Block<'a> {
    Text(TextBlock<'a>),
    /// A box of known size; `content` is drawn with its origin at the box's top left corner.
    Fixed {
        width: f32,
        height: f32,
        content: Svgr<'a>,
    },
    /// Empty space of this many pixels along the stack's direction.
    Gap(f32),
    /// Children one after another along `direction`, `gap` pixels apart, aligned across it.
    Stack {
        direction: Direction,
        gap: f32,
        align: Align,
        children: Vec<Block<'a>>,
    },
    /// A block whose rectangle [`Measured::place`] reports under `name`.
    Named { name: &'a str, block: Box<Block<'a>> },
}

impl<'a> Block<'a> {
    pub fn column(gap: f32, align: Align, children: Vec<Block<'a>>) -> Self {
        Block::Stack { direction: Direction::Column, gap, align, children }
    }

    pub fn row(gap: f32, align: Align, children: Vec<Block<'a>>) -> Self {
        Block::Stack { direction: Direction::Row, gap, align, children }
    }

    pub fn fixed(width: f32, height: f32, content: Svgr<'a>) -> Self {
        Block::Fixed { width, height, content }
    }

    pub fn named(name: &'a str, block: Block<'a>) -> Self {
        Block::Named { name, block: Box::new(block) }
    }

    /// Resolves every text against the fonts. `None` when a font is not resolved (the
    /// editor may still be loading it); callers fall back the way they do for
    /// `text_break_lines`.
    pub fn measure<M: TextMeasurer<'a>>(self, measurer: &mut M) -> Option<Measured<'a>> {
        Some(match self {
            Block::Text(block) => {
                let metrics = measurer.metrics(block.font)?;
                let lines = measurer.wrap(
                    &block.text,
                    block.font,
                    block.max_width.unwrap_or(usize::MAX),
                    block.line_height,
                    block.align,
                )?;
                let widest = lines.lines.iter().map(|line| line.width).max().unwrap_or(0);
                let width = block.max_width.unwrap_or(widest) as f32;
                let height = match lines.lines.len() {
                    0 => 0.0,
                    n => metrics.height() + (n - 1) as f32 * lines.line_height_in_px,
                };
                Measured {
                    size: Size { width, height },
                    kind: Kind::Text { block, lines, metrics },
                }
            }
            Block::Fixed { width, height, content } => Measured {
                size: Size { width, height },
                kind: Kind::Fixed(content),
            },
            Block::Gap(length) => Measured {
                size: Size { width: length, height: length },
                kind: Kind::Gap,
            },
            Block::Stack { direction, gap, align, children } => {
                let children = children
                    .into_iter()
                    .map(|child| child.measure(measurer))
                    .collect::<Option<Vec<_>>>()?;
                let (along, across) = children.iter().fold((0.0, 0.0f32), |(along, across), child| {
                    let (main, cross) = direction.split(child.size);
                    (along + main, across.max(cross))
                });
                let gaps = gap * children.len().saturating_sub(1) as f32;
                Measured {
                    size: direction.join(along + gaps, across),
                    kind: Kind::Stack { direction, gap, align, children },
                }
            }
            Block::Named { name, block } => {
                let inner = block.measure(measurer)?;
                Measured {
                    size: inner.size,
                    kind: Kind::Named { name, inner: Box::new(inner) },
                }
            }
        })
    }
}

impl Direction {
    /// `(along the direction, across it)`.
    fn split(self, size: Size) -> (f32, f32) {
        match self {
            Direction::Column => (size.height, size.width),
            Direction::Row => (size.width, size.height),
        }
    }

    fn join(self, along: f32, across: f32) -> Size {
        match self {
            Direction::Column => Size { width: across, height: along },
            Direction::Row => Size { width: along, height: across },
        }
    }
}

/// Measures text for [`Block::measure`]: a frame and its context in a video
/// ([`FrameMeasurer`]), or a stand-in in tests.
pub trait TextMeasurer<'a> {
    fn wrap(
        &mut self,
        text: &str,
        font: FontQuery<'a>,
        max_width: usize,
        line_height: f32,
        align: TextAlign,
    ) -> Option<WrappedTextStructure>;

    fn metrics(&mut self, font: FontQuery<'a>) -> Option<FontMetrics>;
}

/// The measurer of a video: the frame's text cache and the context's fonts.
pub struct FrameMeasurer<'f, 'a, 'm> {
    pub frame: &'f mut Frame,
    pub ctx: &'f FFramesContext<'a, 'm>,
}

impl<'a> TextMeasurer<'a> for FrameMeasurer<'_, 'a, '_> {
    fn wrap(
        &mut self,
        text: &str,
        font: FontQuery<'a>,
        max_width: usize,
        line_height: f32,
        align: TextAlign,
    ) -> Option<WrappedTextStructure> {
        // Measured at the origin without a colour, so the cache entry serves the text
        // wherever it is placed and whatever its fill.
        self.frame.text_break_lines_structure(
            self.ctx,
            text,
            BreakLinesOpts::<i32, i32> {
                font,
                width: max_width,
                line_height,
                align,
                ..Default::default()
            },
        )
    }

    fn metrics(&mut self, font: FontQuery<'a>) -> Option<FontMetrics> {
        self.frame.text_metrics(self.ctx, font)
    }
}

/// A block after measuring: its size and everything needed to draw it.
pub struct Measured<'a> {
    size: Size,
    kind: Kind<'a>,
}

enum Kind<'a> {
    Text {
        block: TextBlock<'a>,
        lines: WrappedTextStructure,
        metrics: FontMetrics,
    },
    Fixed(Svgr<'a>),
    Gap,
    Stack {
        direction: Direction,
        gap: f32,
        align: Align,
        children: Vec<Measured<'a>>,
    },
    Named {
        name: &'a str,
        inner: Box<Measured<'a>>,
    },
}

/// The rectangle of a named block after placing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placed<'a> {
    pub name: &'a str,
    pub rect: Rect,
}

impl<'a> Measured<'a> {
    pub fn size(&self) -> Size {
        self.size
    }

    /// How many lines of text the block holds, wrapped.
    pub fn line_count(&self) -> usize {
        match &self.kind {
            Kind::Text { lines, .. } => lines.lines.len(),
            Kind::Fixed(_) | Kind::Gap => 0,
            Kind::Stack { children, .. } => children.iter().map(Measured::line_count).sum(),
            Kind::Named { inner, .. } => inner.line_count(),
        }
    }

    /// The top left corner of every child of a stack placed at `(x, y)`.
    fn child_origins(&self, x: f32, y: f32) -> Vec<(f32, f32)> {
        let Kind::Stack { direction, gap, align, children } = &self.kind else {
            return Vec::new();
        };
        let (_, across) = direction.split(self.size);
        let mut cursor = 0.0;
        children
            .iter()
            .map(|child| {
                let (main, cross) = direction.split(child.size);
                let offset = match align {
                    Align::Start => 0.0,
                    Align::Center => (across - cross) / 2.0,
                    Align::End => across - cross,
                };
                let origin = match direction {
                    Direction::Column => (x + offset, y + cursor),
                    Direction::Row => (x + cursor, y + offset),
                };
                cursor += main + gap;
                origin
            })
            .collect()
    }

    /// The rectangles of the named blocks when the tree's top left corner is at `(x, y)`.
    pub fn place(&self, x: f32, y: f32) -> Vec<Placed<'a>> {
        let mut placed = Vec::new();
        self.place_into(x, y, &mut placed);
        placed
    }

    fn place_into(&self, x: f32, y: f32, out: &mut Vec<Placed<'a>>) {
        match &self.kind {
            Kind::Named { name, inner } => {
                out.push(Placed {
                    name,
                    rect: Rect { x, y, width: self.size.width, height: self.size.height },
                });
                inner.place_into(x, y, out);
            }
            Kind::Stack { children, .. } => {
                for (child, (cx, cy)) in children.iter().zip(self.child_origins(x, y)) {
                    child.place_into(cx, cy, out);
                }
            }
            Kind::Text { .. } | Kind::Fixed(_) | Kind::Gap => {}
        }
    }

    /// The rectangle of the first block named `name`, placed at `(x, y)`.
    pub fn rect_of(&self, name: &str, x: f32, y: f32) -> Option<Rect> {
        self.place(x, y).into_iter().find(|p| p.name == name).map(|p| p.rect)
    }

    /// Draws the tree with its top left corner at `(x, y)`.
    pub fn render(self, x: f32, y: f32) -> Svgr<'a> {
        match self.kind {
            Kind::Text { block, lines, metrics } => {
                let baseline = y + metrics.ascent;
                let step = lines.line_height_in_px;
                let width = self.size.width;
                let spans: Svgr<'a> = lines
                    .lines
                    .iter()
                    .enumerate()
                    .map(|(index, line)| {
                        let dx = match block.align {
                            TextAlign::Left => 0.0,
                            TextAlign::Center => (width - line.width as f32) / 2.0,
                            TextAlign::Right => width - line.width as f32,
                        };
                        let line_y = baseline + index as f32 * step;
                        svgr!(<tspan x={x + dx} y={line_y}>{line.words.join(" ")}</tspan>)
                    })
                    .collect();
                let style = match block.font.style {
                    FontStyle::Normal => "normal",
                    FontStyle::Italic => "italic",
                    FontStyle::Oblique => "oblique",
                };
                svgr!(
                    <text
                        font-family={block.font.family}
                        font-size={block.font.size}
                        font-weight={block.font.weight}
                        font-style={style}
                        font-stretch={block.font.stretch.to_string()}
                        fill={block.fill}
                    >
                        {spans}
                    </text>
                )
            }
            Kind::Fixed(content) => svgr!(<g transform={format!("translate({x} {y})")}>{content}</g>),
            Kind::Gap => Svgr::empty(),
            Kind::Stack { .. } => {
                let origins = self.child_origins(x, y);
                let Kind::Stack { children, .. } = self.kind else {
                    unreachable!("matched a stack above");
                };
                children
                    .into_iter()
                    .zip(origins)
                    .map(|(child, (cx, cy))| child.render(cx, cy))
                    .collect()
            }
            Kind::Named { inner, .. } => inner.render(x, y),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::WrappedTextLine;

    /// Ten pixels per character, ascent 0.8 and descent 0.2 of the size, wrapping at spaces.
    struct TenPx;

    impl<'a> TextMeasurer<'a> for TenPx {
        fn wrap(
            &mut self,
            text: &str,
            font: FontQuery<'a>,
            max_width: usize,
            line_height: f32,
            _: TextAlign,
        ) -> Option<WrappedTextStructure> {
            let mut lines: Vec<WrappedTextLine> = Vec::new();
            for word in text.split_whitespace() {
                let width = word.chars().count() * 10;
                match lines.last_mut() {
                    Some(line) if line.width + 10 + width <= max_width => {
                        line.words.push(word.to_owned());
                        line.width += 10 + width;
                    }
                    _ => lines.push(WrappedTextLine { words: vec![word.to_owned()], width, dx: 0, dy: 0 }),
                }
            }
            Some(WrappedTextStructure {
                lines,
                line_height_in_px: line_height * font.size as f32,
                hash: 0,
            })
        }

        fn metrics(&mut self, font: FontQuery<'a>) -> Option<FontMetrics> {
            Some(FontMetrics {
                ascent: font.size as f32 * 0.8,
                descent: font.size as f32 * 0.2,
                line_gap: 0.0,
            })
        }
    }

    fn font() -> FontQuery<'static> {
        FontQuery { family: "Test", size: 20, weight: 400, ..Default::default() }
    }

    #[test]
    fn text_height_comes_from_metrics_and_line_height() {
        let one = Block::Text(TextBlock::new("Hello", font())).measure(&mut TenPx).unwrap();
        assert_eq!(one.size(), Size { width: 50.0, height: 20.0 });
        assert_eq!(one.line_count(), 1);

        // "Hello world again" at 120 px: "Hello world" (110) and "again" (50).
        let two = Block::Text(TextBlock::new("Hello world again", font()).wrap(120).line_height(1.5))
            .measure(&mut TenPx)
            .unwrap();
        assert_eq!(two.line_count(), 2);
        // ascent + descent of the last line plus one baseline step: 20 + 30.
        assert_eq!(two.size(), Size { width: 120.0, height: 50.0 });
    }

    #[test]
    fn stacks_add_along_their_direction_and_align_across() {
        let column = Block::column(
            10.0,
            Align::Center,
            vec![
                Block::named("title", Block::Text(TextBlock::new("Title", font()))),
                Block::Gap(5.0),
                Block::fixed(100.0, 30.0, Svgr::empty()),
            ],
        );
        let measured = column.measure(&mut TenPx).unwrap();
        // widths 50, 5, 100 -> 100; heights 20 + 5 + 30 plus two gaps of 10.
        assert_eq!(measured.size(), Size { width: 100.0, height: 75.0 });
        // the 50 px title is centred in the 100 px column, at the top.
        assert_eq!(
            measured.rect_of("title", 200.0, 300.0),
            Some(Rect { x: 225.0, y: 300.0, width: 50.0, height: 20.0 })
        );

        let row = Block::row(
            0.0,
            Align::End,
            vec![
                Block::fixed(10.0, 40.0, Svgr::empty()),
                Block::named("label", Block::Text(TextBlock::new("ab", font()))),
            ],
        );
        let measured = row.measure(&mut TenPx).unwrap();
        assert_eq!(measured.size(), Size { width: 30.0, height: 40.0 });
        // bottom aligned: the 20 px label sits 20 px down, right of the box.
        assert_eq!(
            measured.rect_of("label", 0.0, 0.0),
            Some(Rect { x: 10.0, y: 20.0, width: 20.0, height: 20.0 })
        );
    }

    #[test]
    fn rendered_text_sits_on_its_baseline() {
        // "Hi" and "there" would need 80 px on one line; at 70 the text wraps.
        let measured = Block::Text(TextBlock::new("Hi there", font()).wrap(70).align(TextAlign::Right))
            .measure(&mut TenPx)
            .unwrap();
        assert_eq!(measured.line_count(), 2);
        let svg = measured.render(100.0, 50.0).to_string();
        // first baseline at y + ascent = 66; "Hi" (20 px) right aligned in 70: x 150.
        assert!(svg.contains(r#"<tspan x="150" y="66">Hi</tspan>"#), "{svg}");
        // second line one step (1.2 * 20 = 24) lower; "there" is 50 px: x 120.
        assert!(svg.contains(r#"<tspan x="120" y="90">there</tspan>"#), "{svg}");
        assert!(svg.contains(r#"font-style="normal""#), "{svg}");
    }
}
