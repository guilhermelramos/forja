//! Endpoint-form ellipse arcs to cubic segments, using SVG's geometric definition.
//! Reference: https://www.w3.org/TR/SVG2/implnote.html#ArcImplementationNotes
use super::path::Command;
use std::f64::consts::{FRAC_PI_2, TAU};
#[allow(clippy::too_many_arguments)]
pub(super) fn convert(
    from: [f64; 2],
    to: [f64; 2],
    rx: f64,
    ry: f64,
    rotation: f64,
    large: bool,
    sweep: bool,
) -> Vec<Command> {
    if from == to {
        return Vec::new();
    }
    let (mut rx, mut ry) = (rx.abs(), ry.abs());
    if rx == 0.0 || ry == 0.0 {
        return vec![Command::Line(to)];
    }
    let (s, c) = rotation.to_radians().sin_cos();
    let (dx, dy) = ((from[0] - to[0]) / 2.0, (from[1] - to[1]) / 2.0);
    let (x, y) = (c * dx + s * dy, -s * dx + c * dy);
    let scale = (x * x / (rx * rx) + y * y / (ry * ry)).sqrt().max(1.0);
    rx *= scale;
    ry *= scale;
    let numerator = (rx * rx * ry * ry - rx * rx * y * y - ry * ry * x * x).max(0.0);
    let denominator = rx * rx * y * y + ry * ry * x * x;
    let factor = if large == sweep { -1.0 } else { 1.0 } * (numerator / denominator).sqrt();
    let (cx0, cy0) = (factor * rx * y / ry, -factor * ry * x / rx);
    let (cx, cy) = (
        c * cx0 - s * cy0 + (from[0] + to[0]) / 2.0,
        s * cx0 + c * cy0 + (from[1] + to[1]) / 2.0,
    );
    let (u, v) = (
        [(x - cx0) / rx, (y - cy0) / ry],
        [(-x - cx0) / rx, (-y - cy0) / ry],
    );
    let start = u[1].atan2(u[0]);
    let mut delta = (u[0] * v[1] - u[1] * v[0]).atan2(u[0] * v[0] + u[1] * v[1]);
    if sweep && delta < 0.0 {
        delta += TAU;
    } else if !sweep && delta > 0.0 {
        delta -= TAU;
    }
    let parts = (delta.abs() / FRAC_PI_2).ceil().max(1.0) as usize;
    let step = delta / parts as f64;
    let point = |t: f64| {
        [
            cx + rx * c * t.cos() - ry * s * t.sin(),
            cy + rx * s * t.cos() + ry * c * t.sin(),
        ]
    };
    let tangent = |t: f64| {
        [
            -rx * c * t.sin() - ry * s * t.cos(),
            -rx * s * t.sin() + ry * c * t.cos(),
        ]
    };
    (0..parts)
        .map(|i| {
            let a = start + i as f64 * step;
            let b = a + step;
            let k = 4.0 / 3.0 * (step / 4.0).tan();
            let (p, q, ta, tb) = (point(a), point(b), tangent(a), tangent(b));
            Command::Cubic(
                [p[0] + k * ta[0], p[1] + k * ta[1]],
                [q[0] - k * tb[0], q[1] - k * tb[1]],
                if i + 1 == parts { to } else { q },
            )
        })
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quarter_circle_has_correct_endpoint_and_tangents() {
        let cmds = convert([4., 12.], [12., 4.], 8., 8., 0., false, true);
        assert_eq!(cmds.len(), 1);
        let Command::Cubic(a, b, end) = cmds[0] else {
            panic!("cubic")
        };
        assert_eq!(end, [12., 4.]);
        assert!((a[0] - 4.).abs() < 1e-6);
        assert!((b[1] - 4.).abs() < 1e-6);
    }
    #[test]
    fn degenerate_and_large_rotated_arcs_remain_finite() {
        assert!(convert([1., 1.], [1., 1.], 3., 4., 0., false, true).is_empty());
        assert_eq!(
            convert([0., 0.], [1., 1.], 0., 2., 0., false, false),
            vec![Command::Line([1., 1.])]
        );
        assert!(convert([2., 3.], [20., 19.], 3., 7., 35., true, false).len() >= 2);
    }
}
