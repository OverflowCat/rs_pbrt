//std
use std::sync::Arc;
// pbrt
use crate::core::interaction::SurfaceInteraction;
use crate::core::material::{Material, TransportMode};
use crate::core::microfacet::{MicrofacetDistribution, TrowbridgeReitzDistribution};
use crate::core::paramset::TextureParams;
use crate::core::pbrt::{Float, Spectrum};
use crate::core::reflection::{
    Bsdf, Bxdf, Fresnel, FresnelDielectric, LambertianReflection, MicrofacetReflection,
    SpecularReflection, SpecularTransmission,
};
use crate::core::texture::Texture;

// see uber.h

pub struct UberMaterial {
    pub kd: Arc<dyn Texture<Spectrum> + Sync + Send>, // default: 0.25
    pub ks: Arc<dyn Texture<Spectrum> + Sync + Send>, // default: 0.25
    pub kr: Arc<dyn Texture<Spectrum> + Sync + Send>, // default: 0.0
    pub kt: Arc<dyn Texture<Spectrum> + Sync + Send>, // default: 0.0
    pub opacity: Arc<dyn Texture<Spectrum> + Sync + Send>, // default: 1.0
    pub roughness: Arc<dyn Texture<Float> + Sync + Send>, // default: 0.1
    pub u_roughness: Option<Arc<dyn Texture<Float> + Sync + Send>>,
    pub v_roughness: Option<Arc<dyn Texture<Float> + Sync + Send>>,
    pub eta: Arc<dyn Texture<Float> + Sync + Send>, // default: 1.5
    pub bump_map: Option<Arc<dyn Texture<Float> + Sync + Send>>,
    pub remap_roughness: bool,
}

impl UberMaterial {
    pub fn new(
        kd: Arc<dyn Texture<Spectrum> + Sync + Send>,
        ks: Arc<dyn Texture<Spectrum> + Sync + Send>,
        kr: Arc<dyn Texture<Spectrum> + Sync + Send>,
        kt: Arc<dyn Texture<Spectrum> + Sync + Send>,
        roughness: Arc<dyn Texture<Float> + Sync + Send>,
        u_roughness: Option<Arc<dyn Texture<Float> + Sync + Send>>,
        v_roughness: Option<Arc<dyn Texture<Float> + Sync + Send>>,
        opacity: Arc<dyn Texture<Spectrum> + Sync + Send>,
        eta: Arc<dyn Texture<Float> + Send + Sync>,
        bump_map: Option<Arc<dyn Texture<Float> + Sync + Send>>,
        remap_roughness: bool,
    ) -> Self {
        UberMaterial {
            kd,
            ks,
            kr,
            kt,
            opacity,
            roughness,
            u_roughness,
            v_roughness,
            eta,
            bump_map,
            remap_roughness,
        }
    }
    pub fn create(mp: &mut TextureParams) -> Arc<Material> {
        let kd: Arc<dyn Texture<Spectrum> + Sync + Send> =
            mp.get_spectrum_texture("Kd", Spectrum::new(0.25));
        let ks: Arc<dyn Texture<Spectrum> + Sync + Send> =
            mp.get_spectrum_texture("Ks", Spectrum::new(0.25));
        let kr: Arc<dyn Texture<Spectrum> + Sync + Send> =
            mp.get_spectrum_texture("Kr", Spectrum::new(0.0));
        let kt: Arc<dyn Texture<Spectrum> + Sync + Send> =
            mp.get_spectrum_texture("Kt", Spectrum::new(0.0));
        let roughness: Arc<dyn Texture<Float> + Send + Sync> =
            mp.get_float_texture("roughness", 0.1 as Float);
        let u_roughness: Option<Arc<dyn Texture<Float> + Send + Sync>> =
            mp.get_float_texture_or_null("uroughness");
        let v_roughness: Option<Arc<dyn Texture<Float> + Send + Sync>> =
            mp.get_float_texture_or_null("vroughness");
        let opacity: Arc<dyn Texture<Spectrum> + Send + Sync> =
            mp.get_spectrum_texture("opacity", Spectrum::new(1.0));
        let bump_map: Option<Arc<dyn Texture<Float> + Send + Sync>> =
            mp.get_float_texture_or_null("bumpmap");
        let remap_roughness: bool = mp.find_bool("remaproughness", true);
        let eta_option: Option<Arc<dyn Texture<Float> + Send + Sync>> =
            mp.get_float_texture_or_null("eta");
        if let Some(ref eta) = eta_option {
            Arc::new(Material::Uber(Box::new(UberMaterial::new(
                kd,
                ks,
                kr,
                kt,
                roughness,
                u_roughness,
                v_roughness,
                opacity,
                eta.clone(),
                bump_map,
                remap_roughness,
            ))))
        } else {
            let eta: Arc<dyn Texture<Float> + Send + Sync> =
                mp.get_float_texture("index", 1.5 as Float);
            Arc::new(Material::Uber(Box::new(UberMaterial::new(
                kd,
                ks,
                kr,
                kt,
                roughness,
                u_roughness,
                v_roughness,
                opacity,
                eta,
                bump_map,
                remap_roughness,
            ))))
        }
    }
    // Material
    pub fn compute_scattering_functions(
        &self,
        si: &mut SurfaceInteraction,
        arena_bsdf: &mut Vec<Bsdf>,
        arena_bxdf: &mut Vec<Bxdf>,
        mode: TransportMode,
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
        let e: Float = self.eta.evaluate(si);
        let op: Spectrum = self
            .opacity
            .evaluate(si)
            .clamp(0.0 as Float, std::f32::INFINITY as Float);
        let t: Spectrum =
            (Spectrum::new(1.0) - op).clamp(0.0 as Float, std::f32::INFINITY as Float);
        let kd: Spectrum = op
            * self
                .kd
                .evaluate(si)
                .clamp(0.0 as Float, std::f32::INFINITY as Float);
        let ks: Spectrum = op
            * self
                .ks
                .evaluate(si)
                .clamp(0.0 as Float, std::f32::INFINITY as Float);
        let mut u_rough: Float;
        if let Some(ref u_roughness) = self.u_roughness {
            u_rough = u_roughness.evaluate(si);
        } else {
            u_rough = self.roughness.evaluate(si);
        }
        let mut v_rough: Float;
        if let Some(ref v_roughness) = self.v_roughness {
            v_rough = v_roughness.evaluate(si);
        } else {
            v_rough = self.roughness.evaluate(si);
        }
        let kr: Spectrum = op
            * self
                .kr
                .evaluate(si)
                .clamp(0.0 as Float, std::f32::INFINITY as Float);
        let kt: Spectrum = op
            * self
                .kt
                .evaluate(si)
                .clamp(0.0 as Float, std::f32::INFINITY as Float);
        let mut bsdf: Bsdf;
        if !t.is_black() {
            bsdf = Bsdf::new(si, 1.0);
        } else {
            bsdf = Bsdf::new(si, e);
        }
        if !t.is_black() {
            if use_scale {
                arena_bxdf.push(Bxdf::SpecTrans(SpecularTransmission::new(
                    t,
                    1.0,
                    1.0,
                    mode,
                    Some(sc),
                )));
                bsdf.add(arena_bxdf.len() - 1);
            } else {
                arena_bxdf.push(Bxdf::SpecTrans(SpecularTransmission::new(
                    t, 1.0, 1.0, mode, None,
                )));
                bsdf.add(arena_bxdf.len() - 1);
            }
        }
        if !kd.is_black() {
            if use_scale {
                arena_bxdf.push(Bxdf::LambertianRefl(LambertianReflection::new(
                    kd,
                    Some(sc),
                )));
                bsdf.add(arena_bxdf.len() - 1);
            } else {
                arena_bxdf.push(Bxdf::LambertianRefl(LambertianReflection::new(kd, None)));
                bsdf.add(arena_bxdf.len() - 1);
            }
        }
        if !ks.is_black() {
            let fresnel = Fresnel::Dielectric(FresnelDielectric {
                eta_i: 1.0,
                eta_t: e,
            });
            if self.remap_roughness {
                u_rough = TrowbridgeReitzDistribution::roughness_to_alpha(u_rough);
                v_rough = TrowbridgeReitzDistribution::roughness_to_alpha(v_rough);
            }
            let distrib = MicrofacetDistribution::TrowbridgeReitz(
                TrowbridgeReitzDistribution::new(u_rough, v_rough, true),
            );
            if use_scale {
                arena_bxdf.push(Bxdf::MicrofacetRefl(MicrofacetReflection::new(
                    ks,
                    distrib,
                    fresnel,
                    Some(sc),
                )));
                bsdf.add(arena_bxdf.len() - 1);
            } else {
                arena_bxdf.push(Bxdf::MicrofacetRefl(MicrofacetReflection::new(
                    ks, distrib, fresnel, None,
                )));
                bsdf.add(arena_bxdf.len() - 1);
            }
        }
        if !kr.is_black() {
            let fresnel = Fresnel::Dielectric(FresnelDielectric {
                eta_i: 1.0,
                eta_t: e,
            });
            if use_scale {
                arena_bxdf.push(Bxdf::SpecRefl(SpecularReflection::new(
                    kr,
                    fresnel,
                    Some(sc),
                )));
                bsdf.add(arena_bxdf.len() - 1);
            } else {
                arena_bxdf.push(Bxdf::SpecRefl(SpecularReflection::new(kr, fresnel, None)));
                bsdf.add(arena_bxdf.len() - 1);
            }
        }
        if !kt.is_black() {
            if use_scale {
                arena_bxdf.push(Bxdf::SpecTrans(SpecularTransmission::new(
                    kt,
                    1.0,
                    e,
                    mode,
                    Some(sc),
                )));
                bsdf.add(arena_bxdf.len() - 1);
            } else {
                arena_bxdf.push(Bxdf::SpecTrans(SpecularTransmission::new(
                    kt, 1.0, e, mode, None,
                )));
                bsdf.add(arena_bxdf.len() - 1);
            }
        }
        arena_bsdf.push(bsdf);
        si.bsdf = Some(arena_bsdf.len() - 1);
    }
}
