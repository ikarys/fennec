//! Where the project map draws its boxes and arrows: positions from the layout's
//! layers and rows, arrows routed at right angles through the gaps between boxes,
//! with rounded corners and a hop where two arrows cross.

use std::collections::HashMap;
use std::fmt::Write;

use super::{Dependency, Module, Point};

pub const NODE_WIDTH: i32 = 260;
pub const NODE_HEIGHT: i32 = 46;

const COLUMN: i32 = 340;
const LAYER: i32 = 120;
const RADIUS: i32 = 8;
const HOP: i32 = 4;
const LANE: i32 = 7;

pub fn route(modules: &mut [Module], dependencies: &mut [Dependency]) {
    for m in modules.iter_mut() {
        m.x = m.row as i32 * COLUMN;
        m.y = m.layer as i32 * LAYER;
    }
    let index: HashMap<&str, &Module> = modules.iter().map(|m| (m.id.as_str(), m)).collect();
    let gutter_centres: Vec<Option<i32>> = dependencies
        .iter()
        .map(|d| {
            let src = index[d.from.as_str()];
            let dst = index[d.to.as_str()];
            if dst.layer == src.layer + 1 {
                None
            } else {
                let half_gutter = (COLUMN - NODE_WIDTH) / 2;
                Some(if src.x <= dst.x {
                    dst.x - half_gutter
                } else {
                    dst.x + NODE_WIDTH + half_gutter
                })
            }
        })
        .collect();
    let mut gutter_lane: HashMap<i32, usize> = HashMap::new();
    let gutter_xs: Vec<Option<i32>> = gutter_centres
        .iter()
        .map(|centre| match centre {
            Some(c) => {
                let k = gutter_lane.entry(*c).or_insert(0);
                let lane = *k;
                *k += 1;
                Some(c - (COLUMN - NODE_WIDTH) / 2 + 8 + LANE * (lane % 9) as i32)
            }
            None => None,
        })
        .collect();
    let lines: Vec<Vec<Point>> = dependencies
        .iter()
        .zip(gutter_xs.iter())
        .map(|(d, gutter)| polyline(index[d.from.as_str()], index[d.to.as_str()], *gutter))
        .collect();
    for (i, dep) in dependencies.iter_mut().enumerate() {
        let points = &lines[i];
        dep.path = svg_path(points, &hops(points, &lines, i));
        let dst = index[dep.to.as_str()];
        dep.label = Point {
            x: dst.x + NODE_WIDTH / 2,
            y: dst.y - 14,
        };
    }
}

fn polyline(src: &Module, dst: &Module, gutter: Option<i32>) -> Vec<Point> {
    let (sx, sy) = (src.x + NODE_WIDTH / 2, src.y + NODE_HEIGHT);
    let (tx, ty) = (dst.x + NODE_WIDTH / 2, dst.y);
    let below = src.y + NODE_HEIGHT + LANE * (1 + (src.row % 8) as i32);
    let above = dst.y - LANE * (1 + (dst.row % 8) as i32);
    let raw = if dst.layer == src.layer + 1 {
        vec![pt(sx, sy), pt(sx, below), pt(tx, below), pt(tx, ty)]
    } else {
        let gx = gutter.expect("long arrow has a gutter");
        vec![
            pt(sx, sy),
            pt(sx, below),
            pt(gx, below),
            pt(gx, above),
            pt(tx, above),
            pt(tx, ty),
        ]
    };
    simplify(raw)
}

fn simplify(points: Vec<Point>) -> Vec<Point> {
    let mut out = Vec::new();
    for p in points {
        if !out.last().is_some_and(|q| *q == p) {
            if out.len() >= 2 && collinear(out[out.len() - 2], out[out.len() - 1], p) {
                out.pop();
            }
            out.push(p);
        }
    }
    out
}

fn collinear(a: Point, b: Point, c: Point) -> bool {
    (b.x - a.x) * (c.y - a.y) == (b.y - a.y) * (c.x - a.x)
}

fn hops(self_line: &[Point], all: &[Vec<Point>], self_index: usize) -> Vec<Point> {
    let mut found = Vec::new();
    for s in 0..self_line.len().saturating_sub(1) {
        let (a, b) = (self_line[s], self_line[s + 1]);
        if a.y != b.y || a.x == b.x {
            continue;
        }
        let (lo, hi) = (a.x.min(b.x), a.x.max(b.x));
        for (i, other) in all.iter().enumerate() {
            if i == self_index {
                continue;
            }
            for v in 0..other.len().saturating_sub(1) {
                let (c, d) = (other[v], other[v + 1]);
                if c.x != d.x || c.y == d.y {
                    continue;
                }
                let (x, y) = (c.x, a.y);
                if !(x > lo && x < hi) {
                    continue;
                }
                if !(y > c.y.min(d.y) && y < c.y.max(d.y)) {
                    continue;
                }
                if x - lo < RADIUS + HOP || hi - x < RADIUS + HOP {
                    continue;
                }
                found.push(pt(x, y));
            }
        }
    }
    found
}

fn svg_path(points: &[Point], hops: &[Point]) -> String {
    let first = points[0];
    let mut out = format!("M {} {}", first.x, first.y);
    let mut current = first;
    for i in 0..points.len() - 1 {
        let end = points[i + 1];
        if current.y == end.y && current.x != end.x {
            let dir = if end.x > current.x { 1 } else { -1 };
            let (lo, hi) = (current.x.min(end.x), current.x.max(end.x));
            let mut xs: Vec<i32> = hops
                .iter()
                .filter(|h| h.y == current.y && h.x > lo && h.x < hi)
                .map(|h| h.x)
                .collect();
            xs.sort();
            if dir < 0 {
                xs.reverse();
            }
            for x in xs {
                let sweep = if dir > 0 { 1 } else { 0 };
                let _ = write!(
                    out,
                    " L {} {} A {} {} 0 0 {} {} {}",
                    x - dir * HOP,
                    current.y,
                    HOP,
                    HOP,
                    sweep,
                    x + dir * HOP,
                    current.y
                );
            }
        }
        if i == points.len() - 2 {
            let _ = write!(out, " L {} {}", end.x, end.y);
            break;
        }
        let corner = end;
        let next = points[i + 2];
        let r = radius(points[i], corner, next);
        let before = shift(corner, points[i], r);
        let after = shift(corner, next, r);
        let _ = write!(
            out,
            " L {} {} Q {} {} {} {}",
            before.x, before.y, corner.x, corner.y, after.x, after.y
        );
        current = after;
    }
    out
}

fn radius(a: Point, b: Point, c: Point) -> i32 {
    RADIUS.min(length(a, b) / 2).min(length(b, c) / 2)
}

fn length(a: Point, b: Point) -> i32 {
    (a.x - b.x).abs().max((a.y - b.y).abs())
}

fn shift(from: Point, to: Point, dist: i32) -> Point {
    Point {
        x: from.x + (to.x - from.x).signum() * dist,
        y: from.y + (to.y - from.y).signum() * dist,
    }
}

fn pt(x: i32, y: i32) -> Point {
    Point { x, y }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(modules: &[(&str, usize, usize)], deps: &[(&str, &str)]) -> (Vec<Module>, Vec<String>) {
        let mut ms: Vec<Module> = modules
            .iter()
            .map(|(id, layer, row)| Module {
                id: (*id).into(),
                layer: *layer,
                row: *row,
                ..Default::default()
            })
            .collect();
        let mut ds: Vec<Dependency> = deps
            .iter()
            .map(|(from, to)| Dependency {
                from: (*from).into(),
                to: (*to).into(),
                imports: 1,
                path: String::new(),
                label: Point { x: 0, y: 0 },
                violation: false,
            })
            .collect();
        route(&mut ms, &mut ds);
        (ms, ds.iter().map(|d| d.path.clone()).collect())
    }

    #[test]
    fn positions() {
        let (ms, _) = run(&[("a", 2, 1)], &[]);
        assert_eq!((ms[0].x, ms[0].y), (340, 240));
    }

    #[test]
    fn straight_down() {
        let (_, paths) = run(&[("a", 0, 0), ("b", 1, 0)], &[("a", "b")]);
        assert_eq!(paths[0], "M 130 46 L 130 120");
    }

    #[test]
    fn rounded_step() {
        let (_, paths) = run(&[("a", 0, 0), ("b", 1, 1)], &[("a", "b")]);
        let path = &paths[0];
        assert!(path.starts_with("M 130 46 L 130 "));
        assert_eq!(path.matches(" Q ").count(), 2);
        assert!(path.ends_with("L 470 120"));
    }

    #[test]
    fn long_arrow_uses_gutter() {
        let (_, paths) = run(&[("a", 0, 0), ("b", 2, 1)], &[("a", "b")]);
        assert!(paths[0].contains(" 268 "));
    }

    #[test]
    fn hop_at_crossing() {
        let (_, paths) = run(
            &[("a", 0, 0), ("b", 0, 1), ("c", 1, 0), ("e", 1, 2)],
            &[("a", "e"), ("b", "c")],
        );
        assert!(paths[0].contains("L 466 53 A 4 4 0 0 1 474 53"));
        assert!(!paths[1].contains(" A "));
    }

    #[test]
    fn no_hop_without_crossing() {
        let (_, paths) = run(&[("a", 0, 0), ("b", 1, 1)], &[("a", "b")]);
        assert!(!paths[0].contains(" A "));
    }

    #[test]
    fn parallel_lanes() {
        let (_, paths) = run(
            &[("a", 0, 0), ("b", 0, 1), ("c", 2, 1)],
            &[("a", "c"), ("b", "c")],
        );
        assert!(paths[0].contains(" 268 "));
        assert!(paths[1].contains(" 275 "));
    }
}
