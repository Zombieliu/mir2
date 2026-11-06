//! Crystal BigMapViewPort.OnBeforeDraw geometry, shared by drawing and input.
use super::{BigMapModel, BigMapPoint, BigMapView};
use std::{collections::BTreeMap, sync::OnceLock};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BigMapImageGeometry {
    pub left: f32,
    pub top: f32,
    pub width: f32,
    pub height: f32,
}

impl BigMapImageGeometry {
    pub const WORLD: Self = Self {
        left: 14.,
        top: 52.,
        width: 568.,
        height: 380.,
    };

    pub fn for_image(index: u32) -> Option<Self> {
        static SIZES: OnceLock<BTreeMap<u32, (u32, u32)>> = OnceLock::new();
        let sizes = SIZES.get_or_init(|| {
            let meta: serde_json::Value = serde_json::from_str(include_str!(
                "../../../web/public/original-ui/MMap/meta.json"
            ))
            .expect("exported Crystal MMap metadata");
            meta["frames"]
                .as_array()
                .expect("MMap frames")
                .iter()
                .filter_map(|f| {
                    Some((
                        u32::try_from(f["index"].as_u64()?).ok()?,
                        (
                            u32::try_from(f["width"].as_u64()?).ok()?,
                            u32::try_from(f["height"].as_u64()?).ok()?,
                        ),
                    ))
                })
                .collect()
        });
        let &(width, height) = sizes.get(&index)?;
        if index == 0 || width == 0 || height == 0 {
            return None;
        }
        let (width, height) = (width.min(568), height.min(380));
        Some(Self {
            left: (14 + (568 - width) / 2) as f32,
            top: (52 + (380 - height) / 2) as f32,
            width: width as f32,
            height: height as f32,
        })
    }

    pub fn for_model(model: &BigMapModel) -> Option<Self> {
        if model.view == BigMapView::WorldMap {
            return Some(Self::WORLD);
        }
        Self::for_image(model.active_map()?.info.big_map_image_index()?)
    }

    pub fn image_point(self, panel_point: (f32, f32)) -> Option<(f32, f32)> {
        let (x, y) = (panel_point.0 - self.left, panel_point.1 - self.top);
        ((0. ..self.width).contains(&x) && (0. ..self.height).contains(&y)).then_some((x, y))
    }

    pub fn tile_at(self, point: (f32, f32), width: i32, height: i32) -> Option<(i32, i32)> {
        if width <= 0
            || height <= 0
            || !(0. ..self.width).contains(&point.0)
            || !(0. ..self.height).contains(&point.1)
        {
            return None;
        }
        Some((
            (point.0 * width as f32 / self.width).floor() as i32,
            (point.1 * height as f32 / self.height).floor() as i32,
        ))
    }

    pub fn view_position(self, point: BigMapPoint, width: i32, height: i32) -> (f32, f32) {
        if width <= 0 || height <= 0 {
            return (self.width / 2., self.height / 2.);
        }
        (
            (point.x.max(0) as f32 / width as f32).clamp(0., 1.) * self.width,
            (point.y.max(0) as f32 / height as f32).clamp(0., 1.) * self.height,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_small_large_and_missing_images_share_exact_integer_geometry() {
        for (index, rect) in [
            (8, (148., 142., 300., 199.)),
            (14, (222., 192., 152., 99.)),
            (101, (14., 52., 568., 380.)),
        ] {
            let g = BigMapImageGeometry::for_image(index).unwrap();
            assert_eq!((g.left, g.top, g.width, g.height), rect);
            let point = g.view_position(BigMapPoint { x: 350, y: 350 }, 700, 700);
            assert_eq!(g.tile_at(point, 700, 700), Some((350, 350)));
            assert_eq!(
                g.image_point((g.left + point.0, g.top + point.1)),
                Some(point)
            );
            assert_eq!(g.image_point((g.left - 1., g.top)), None);
            assert_eq!(g.image_point((g.left + g.width, g.top)), None);
        }
        assert_eq!(BigMapImageGeometry::for_image(0), None);
        assert_eq!(BigMapImageGeometry::for_image(u32::MAX), None);
    }
}
