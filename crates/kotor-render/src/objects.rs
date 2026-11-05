//! Doors and placeables from an area's GIT, placed into a scene.

use crate::scene::Scene;
use kotor_archive::Install;
use kotor_core::{ResType, Result};
use kotor_gff::{Gff, Struct};
use kotor_model::{parse_model, Model};
use kotor_tables::TwoDa;
use std::collections::HashMap;

/// What `add_area_objects` did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ObjectStats {
    pub doors_listed: usize,
    pub doors_drawn: usize,
    pub placeables_listed: usize,
    pub placeables_drawn: usize,
    /// Objects whose template, appearance row or model could not be found.
    pub unresolved: usize,
}

/// Z rotation by `bearing` radians, then translation to `(x, y, z)`.
fn placement(x: f32, y: f32, z: f32, bearing: f32) -> [[f32; 4]; 4] {
    let (s, c) = bearing.sin_cos();
    [
        [c, -s, 0.0, x],
        [s, c, 0.0, y],
        [0.0, 0.0, 1.0, z],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

struct Loader<'a> {
    inst: &'a mut Install,
    models: HashMap<String, Option<Model>>,
}

impl Loader<'_> {
    fn model(&mut self, name: &str) -> Option<&Model> {
        let key = name.to_ascii_lowercase();
        if !self.models.contains_key(&key) {
            let m = self.inst.read(&key, ResType::MDL).ok().and_then(|mdl| {
                let mdx = self.inst.read(&key, ResType::MDX).unwrap_or_default();
                parse_model(&mdl, &mdx).ok()
            });
            self.models.insert(key.clone(), m);
        }
        self.models[&key].as_ref()
    }
}

fn table(inst: &mut Install, name: &str) -> Option<TwoDa> {
    TwoDa::parse(&inst.read(name, ResType::TWODA).ok()?).ok()
}

fn f(s: &Struct, label: &str) -> f32 {
    s.float(label).unwrap_or(0.0) as f32
}

/// Add every door and placeable of the area's GIT. The GIT, UTD and UTP are read through `inst`, so the
/// module that owns the area must already be loaded; if there is no GIT nothing is added.
///
/// Doors use `GenericType` (K1) as a row of `genericdoors.2da`; placeables use `Appearance` as a row of
/// `placeables.2da`. Objects are drawn in their model's rest pose (doors closed is assumed, not checked).
pub fn add_area_objects(scene: &mut Scene, inst: &mut Install, area: &str) -> Result<ObjectStats> {
    let mut stats = ObjectStats::default();
    let Ok(git_bytes) = inst.read(area, ResType::GIT) else {
        return Ok(stats);
    };
    let git = Gff::parse(&git_bytes)?;
    let doors2da = table(inst, "genericdoors");
    let plc2da = table(inst, "placeables");
    let mut loader = Loader {
        inst,
        models: HashMap::new(),
    };

    let place = |scene: &mut Scene,
                 loader: &mut Loader,
                 o: &Struct,
                 template_type: ResType,
                 row_field: &str,
                 tda: &Option<TwoDa>|
     -> Option<bool> {
        let tpl = o.str("TemplateResRef")?;
        let utx = Gff::parse(&loader.inst.read(tpl, template_type).ok()?).ok()?;
        let row = utx.root.int(row_field)? as usize;
        let name = tda.as_ref()?.get(row, "modelname")?.to_string();
        let xf = placement(f(o, "X"), f(o, "Y"), f(o, "Z"), f(o, "Bearing"));
        let before = scene.meshes.len();
        let model = loader.model(&name)?;
        scene.add_model_with(model, None, &xf, &[], None);
        Some(scene.meshes.len() > before)
    };

    for d in git.root.list("Door List").unwrap_or_default() {
        stats.doors_listed += 1;
        match place(
            scene,
            &mut loader,
            d,
            ResType(2042),
            "GenericType",
            &doors2da,
        ) {
            Some(true) => stats.doors_drawn += 1,
            Some(false) => {}
            None => stats.unresolved += 1,
        }
    }
    for p in git.root.list("Placeable List").unwrap_or_default() {
        stats.placeables_listed += 1;
        match place(scene, &mut loader, p, ResType(2044), "Appearance", &plc2da) {
            Some(true) => stats.placeables_drawn += 1,
            Some(false) => {}
            None => stats.unresolved += 1,
        }
    }
    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placement_rotates_then_translates() {
        let m = placement(10.0, 0.0, 1.0, std::f32::consts::FRAC_PI_2);
        // The model's +x axis ends up pointing along +y.
        let p = [m[0][0] + m[0][3], m[1][0] + m[1][3]];
        assert!((p[0] - 10.0).abs() < 1e-5 && (p[1] - 1.0).abs() < 1e-5);
        assert_eq!(m[2][3], 1.0);
    }
}
