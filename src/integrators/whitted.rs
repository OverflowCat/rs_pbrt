// std
use std::sync::Arc;
// pbrt
use crate::core::camera::Camera;
use crate::core::geometry::{vec3_abs_dot_nrmf, vec3_dot_nrmf};
use crate::core::geometry::{Bounds2i, Normal3f, Ray, RayDifferential, Vector3f};
use crate::core::interaction::{Interaction, InteractionCommon, SurfaceInteraction};
use crate::core::light::VisibilityTester;
use crate::core::material::TransportMode;
use crate::core::pbrt::{Float, Spectrum};
use crate::core::reflection::{Bsdf, Bxdf, BxdfType};
use crate::core::sampler::Sampler;
use crate::core::scene::Scene;

// see whitted.h

/// Whitted’s ray-tracing algorithm
pub struct WhittedIntegrator {
    // inherited from SamplerIntegrator (see integrator.h)
    pub camera: Arc<Camera>,
    pub sampler: Box<Sampler>,
    pixel_bounds: Bounds2i,
    // see whitted.h
    max_depth: u32,
}

impl WhittedIntegrator {
    pub fn new(
        max_depth: u32,
        camera: Arc<Camera>,
        sampler: Box<Sampler>,
        pixel_bounds: Bounds2i,
    ) -> Self {
        WhittedIntegrator {
            camera,
            sampler,
            pixel_bounds,
            max_depth,
        }
    }
    pub fn preprocess(&mut self, _scene: &Scene) {}
    pub fn li(
        &self,
        ray: &mut Ray,
        scene: &Scene,
        sampler: &mut Sampler,
        arena_bsdf: &mut Vec<Bsdf>,
        arena_bxdf: &mut Vec<Bxdf>,
        depth: i32,
    ) -> Spectrum {
        let mut l: Spectrum = Spectrum::default();
        // find closest ray intersection or return background radiance
        let mut isect: SurfaceInteraction = SurfaceInteraction::default();
        if scene.intersect(ray, &mut isect) {
            // compute emitted and reflected light at ray intersection point

            // initialize common variables for Whitted integrator
            let n: Normal3f = isect.shading.n;
            let wo: Vector3f = isect.common.wo;

            // compute scattering functions for surface interaction
            let mode: TransportMode = TransportMode::Radiance;
            isect.compute_scattering_functions(ray, arena_bsdf, arena_bxdf, false, mode);
            // if (!isect.bsdf)
            if let Some(ref _bsdf) = isect.bsdf {
            } else {
                return self.li(
                    &mut isect.spawn_ray(&ray.d),
                    scene,
                    sampler,
                    arena_bsdf,
                    arena_bxdf,
                    depth,
                );
            }
            // compute emitted light if ray hit an area light source
            l += isect.le(&wo);

            // add contribution of each light source
            for light in &scene.lights {
                let mut light_intr: InteractionCommon = InteractionCommon::default();
                let mut wi: Vector3f = Vector3f::default();
                let mut pdf: Float = 0.0 as Float;
                let mut visibility: VisibilityTester = VisibilityTester::default();
                let li: Spectrum = light.sample_li(
                    &isect.common,
                    &mut light_intr,
                    sampler.get_2d(),
                    &mut wi,
                    &mut pdf,
                    &mut visibility,
                );
                if li.is_black() || pdf == 0.0 as Float {
                    continue;
                }
                if let Some(ref bsdf) = isect.get_bsdf(arena_bsdf) {
                    let bsdf_flags: u8 = BxdfType::BsdfAll as u8;
                    let f: Spectrum = bsdf.f(&wo, &wi, bsdf_flags, arena_bxdf);
                    if !f.is_black() && visibility.unoccluded(scene) {
                        l += f * li * vec3_abs_dot_nrmf(&wi, &n) / pdf;
                    }
                } else {
                    panic!("no isect.bsdf found");
                }
            }
            if depth as u32 + 1 < self.max_depth {
                // trace rays for specular reflection and refraction
                l += self
                    .specular_reflect(ray, &isect, scene, sampler, arena_bsdf, arena_bxdf, depth);
                l += self
                    .specular_transmit(ray, &isect, scene, sampler, arena_bsdf, arena_bxdf, depth);
            }
            l
        } else {
            for light in &scene.lights {
                l += light.le(ray);
            }
            l
        }
    }
    pub fn get_camera(&self) -> Arc<Camera> {
        self.camera.clone()
    }
    pub fn get_sampler(&self) -> &Sampler {
        &self.sampler
    }
    pub fn get_pixel_bounds(&self) -> Bounds2i {
        self.pixel_bounds
    }
    pub fn specular_reflect(
        &self,
        ray: &Ray,
        isect: &SurfaceInteraction,
        scene: &Scene,
        sampler: &mut Sampler,
        arena_bsdf: &mut Vec<Bsdf>,
        arena_bxdf: &mut Vec<Bxdf>,
        depth: i32,
    ) -> Spectrum {
        // compute specular reflection direction _wi_ and BSDF value
        let wo: Vector3f = isect.common.wo;
        let mut wi: Vector3f = Vector3f::default();
        let mut pdf: Float = 0.0 as Float;
        let ns: Normal3f = isect.shading.n;
        let mut sampled_type: u8 = 0_u8;
        let bsdf_flags: u8 = BxdfType::BsdfReflection as u8 | BxdfType::BsdfSpecular as u8;
        let f: Spectrum;
        if let Some(ref bsdf) = isect.get_bsdf(arena_bsdf) {
            f = bsdf.sample_f(
                &wo,
                &mut wi,
                sampler.get_2d(),
                &mut pdf,
                bsdf_flags,
                &mut sampled_type,
                arena_bxdf,
            );
            if pdf > 0.0 as Float && !f.is_black() && vec3_abs_dot_nrmf(&wi, &ns) != 0.0 as Float {
                // compute ray differential _rd_ for specular reflection
                let mut rd: Ray = isect.spawn_ray(&wi);
                if let Some(d) = ray.differential.iter().next() {
                    let dndx: Normal3f = isect.shading.dndu * isect.dudx.get()
                        + isect.shading.dndv * isect.dvdx.get();
                    let dndy: Normal3f = isect.shading.dndu * isect.dudy.get()
                        + isect.shading.dndv * isect.dvdy.get();
                    let dwodx: Vector3f = -d.rx_direction - wo;
                    let dwody: Vector3f = -d.ry_direction - wo;
                    let ddndx: Float = vec3_dot_nrmf(&dwodx, &ns) + vec3_dot_nrmf(&wo, &dndx);
                    let ddndy: Float = vec3_dot_nrmf(&dwody, &ns) + vec3_dot_nrmf(&wo, &dndy);
                    // compute differential reflected directions
                    let diff: RayDifferential = RayDifferential {
                        rx_origin: isect.common.p + isect.dpdx.get(),
                        ry_origin: isect.common.p + isect.dpdy.get(),
                        rx_direction: wi - dwodx
                            + Vector3f::from(dndx * vec3_dot_nrmf(&wo, &ns) + ns * ddndx)
                                * 2.0 as Float,
                        ry_direction: wi - dwody
                            + Vector3f::from(dndy * vec3_dot_nrmf(&wo, &ns) + ns * ddndy)
                                * 2.0 as Float,
                    };
                    rd.differential = Some(diff);
                }
                f * self.li(&mut rd, scene, sampler, arena_bsdf, arena_bxdf, depth + 1)
                    * Spectrum::new(vec3_abs_dot_nrmf(&wi, &ns) / pdf)
            } else {
                Spectrum::new(0.0)
            }
        } else {
            Spectrum::new(0.0)
        }
    }
    pub fn specular_transmit(
        &self,
        ray: &Ray,
        isect: &SurfaceInteraction,
        scene: &Scene,
        sampler: &mut Sampler,
        arena_bsdf: &mut Vec<Bsdf>,
        arena_bxdf: &mut Vec<Bxdf>,
        depth: i32,
    ) -> Spectrum {
        let wo: Vector3f = isect.common.wo;
        let mut wi: Vector3f = Vector3f::default();
        let mut pdf: Float = 0.0 as Float;
        // let p: Point3f = isect.p;
        let ns: Normal3f = isect.shading.n;
        let mut sampled_type: u8 = 0_u8;
        let bsdf_flags: u8 = BxdfType::BsdfTransmission as u8 | BxdfType::BsdfSpecular as u8;
        let f: Spectrum;
        if let Some(ref bsdf) = isect.get_bsdf(arena_bsdf) {
            f = bsdf.sample_f(
                &wo,
                &mut wi,
                sampler.get_2d(),
                &mut pdf,
                bsdf_flags,
                &mut sampled_type,
                arena_bxdf,
            );
            if pdf > 0.0 as Float && !f.is_black() && vec3_abs_dot_nrmf(&wi, &ns) != 0.0 as Float {
                // compute ray differential _rd_ for specular transmission
                let mut rd: Ray = isect.spawn_ray(&wi);
                if let Some(d) = ray.differential.iter().next() {
                    let mut eta: Float = bsdf.eta;
                    let w: Vector3f = -wo;
                    if vec3_dot_nrmf(&wo, &ns) < 0.0 as Float {
                        eta = 1.0 / eta;
                    }
                    let dndx: Normal3f = isect.shading.dndu * isect.dudx.get()
                        + isect.shading.dndv * isect.dvdx.get();
                    let dndy: Normal3f = isect.shading.dndu * isect.dudy.get()
                        + isect.shading.dndv * isect.dvdy.get();
                    let dwodx: Vector3f = -d.rx_direction - wo;
                    let dwody: Vector3f = -d.ry_direction - wo;
                    let ddndx: Float = vec3_dot_nrmf(&dwodx, &ns) + vec3_dot_nrmf(&wo, &dndx);
                    let ddndy: Float = vec3_dot_nrmf(&dwody, &ns) + vec3_dot_nrmf(&wo, &dndy);
                    let mu: Float = eta * vec3_dot_nrmf(&w, &ns) - vec3_dot_nrmf(&wi, &ns);
                    let dmudx: Float = (eta
                        - (eta * eta * vec3_dot_nrmf(&w, &ns)) / vec3_dot_nrmf(&wi, &ns))
                        * ddndx;
                    let dmudy: Float = (eta
                        - (eta * eta * vec3_dot_nrmf(&w, &ns)) / vec3_dot_nrmf(&wi, &ns))
                        * ddndy;
                    let diff: RayDifferential = RayDifferential {
                        rx_origin: isect.common.p + isect.dpdx.get(),
                        ry_origin: isect.common.p + isect.dpdy.get(),
                        rx_direction: wi + dwodx * eta - Vector3f::from(dndx * mu + ns * dmudx),
                        ry_direction: wi + dwody * eta - Vector3f::from(dndy * mu + ns * dmudy),
                    };
                    rd.differential = Some(diff);
                }
                f * self.li(&mut rd, scene, sampler, arena_bsdf, arena_bxdf, depth + 1)
                    * Spectrum::new(vec3_abs_dot_nrmf(&wi, &ns) / pdf)
            } else {
                Spectrum::new(0.0)
            }
        } else {
            Spectrum::new(0.0)
        }
    }
}
