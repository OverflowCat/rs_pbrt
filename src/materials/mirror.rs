//std
use std::sync::Arc;
// pbrt
use crate::core::interaction::SurfaceInteraction;
use crate::core::material::{Material, TransportMode};
use crate::core::paramset::TextureParams;
use crate::core::pbrt::{Float, Spectrum};
use crate::core::reflection::{Bsdf, Bxdf, Fresnel, FresnelNoOp, SpecularReflection};
use crate::core::texture::Texture;

// see mirror.h

/// A simple mirror, modeled with perfect specular reflection.
pub struct MirrorMaterial {
    pub kr: Arc<dyn Texture<Spectrum> + Sync + Send>, // default: 0.9
    pub bump_map: Option<Arc<dyn Texture<Float> + Send + Sync>>,
}

impl MirrorMaterial {
    pub fn new(
        kr: Arc<dyn Texture<Spectrum> + Send + Sync>,
        bump_map: Option<Arc<dyn Texture<Float> + Sync + Send>>,
    ) -> Self {
        MirrorMaterial { kr, bump_map }
    }
    pub fn create(mp: &mut TextureParams) -> Arc<Material> {
        let kr = mp.get_spectrum_texture("Kr", Spectrum::new(0.9 as Float));
        let bump_map = mp.get_float_texture_or_null("bumpmap");
        Arc::new(Material::Mirror(Box::new(MirrorMaterial::new(
            kr, bump_map,
        ))))
    }
    // Material
    pub fn compute_scattering_functions(
        &self,
        si: &mut SurfaceInteraction,
        arena: &mut Vec<Bsdf>,
        _mode: TransportMode,
        _allow_multiple_lobes: bool,
        _material: Option<Arc<Material>>,
        scale_opt: Option<Spectrum>,
    ) {
        let mut use_scale: bool = false;
        let mut sc: Spectrum = Spectrum::default();
        if let Some(scale) = scale_opt {
            use_scale = true;
            sc = scale;
        }
        if let Some(ref bump) = self.bump_map {
            Material::bump(bump, si);
        }
        let r: Spectrum = self
            .kr
            .evaluate(si)
            .clamp(0.0 as Float, std::f32::INFINITY as Float);
        let mut bsdf = Bsdf::new(si, 1.0);
        let fresnel = Fresnel::NoOp(FresnelNoOp {});
        if use_scale {
            bsdf.add(Bxdf::SpecRefl(SpecularReflection::new(
                r,
                fresnel,
                Some(sc),
            )));
        } else {
            bsdf.add(Bxdf::SpecRefl(SpecularReflection::new(r, fresnel, None)));
        }
        arena.push(bsdf);
        si.bsdf = Some(arena.len() - 1);
    }
}
