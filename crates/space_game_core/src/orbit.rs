use std::f64::consts::TAU;

use nalgebra::{Point3, Vector3, Matrix3x2, Vector2};

pub struct OrbitalElements {
    pub semi_major_axis: f64,
    pub eccentricity: f64,
    pub argument_of_periapsis: f64,
    pub longitude_of_ascending_node: f64,
    pub inclination: f64,
    pub mean_anomaly: f64,
}

pub struct StateVector {
    pub position: Point3<f64>,
    pub velocity: Vector3<f64>,
}

pub const GRAVITATIONAL_CONSTANT: f64 = 6.6743015e-11;

impl OrbitalElements {
    pub fn from_state_vector(sv: &StateVector, central_body_mass: f64) -> Self {
        let gravitational_parameter = GRAVITATIONAL_CONSTANT * central_body_mass;
        let pos = sv.position.coords;
        let vel = sv.velocity;

        let momentum = pos.cross(&vel);
        let eccentricity_vec = (vel.cross(&momentum) / gravitational_parameter)
            - pos.normalize();
        let eccentricity = eccentricity_vec.norm();
        let nvec = Vector3::new(-momentum.y, momentum.x, 0.0);
        let true_anomaly = fix_quadrant(
            (eccentricity_vec.dot(&pos) / (eccentricity * pos.norm())).acos(),
            pos.dot(&vel),
        );
        let inclination = momentum.z / momentum.norm();
        let eccentric_anomaly = 2.0 * 
            (true_anomaly / 2.0).tan().atan2(
                ((1.0 + eccentricity) / (1.0 - eccentricity)).sqrt());
        let longitude_of_ascending_node = fix_quadrant(
            nvec.x / nvec.norm(),
            nvec.y,
        );
        let argument_of_periapsis = fix_quadrant(
            nvec.dot(&eccentricity_vec) / (nvec.norm() * eccentricity),
            eccentricity_vec.z,
        );
        let mean_anomaly = eccentric_anomaly - eccentricity * eccentric_anomaly.sin();
        let semi_major_axis = 1.0 / ((2.0 / pos.norm()) - (vel.norm_squared() / gravitational_parameter));
        OrbitalElements { semi_major_axis, eccentricity, argument_of_periapsis, longitude_of_ascending_node, inclination, mean_anomaly }
    }

    fn as_state_vector(&self, central_body_mass: f64) -> StateVector {
        let gravitational_parameter = GRAVITATIONAL_CONSTANT * central_body_mass;
        let eccentric_anomaly = {
            let mut result = self.mean_anomaly;
            for _ in 0..10 {
                result -= (result * self.eccentricity * result.sin() - self.mean_anomaly) / (1.0 - self.eccentricity * result.cos());
            }
            result
        };
        let true_anomaly = {
            let e_div_2 = eccentric_anomaly / 2.0;
            2.0 * ((1.0 + self.eccentricity).sqrt() * e_div_2.sin())
                .atan2((1.0 - self.eccentricity).sqrt() * e_div_2.cos())
        };
        let distance = self.semi_major_axis * (1.0 - self.eccentricity * eccentric_anomaly.cos());
        let orbit_pos = distance * Vector2::new(true_anomaly.cos(), true_anomaly.sin());
        let orbit_vel = (gravitational_parameter * self.semi_major_axis).sqrt() / distance *
            Vector2::new(-eccentric_anomaly.sin(), (1.0 - eccentric_anomaly * eccentric_anomaly).sqrt() * eccentric_anomaly.cos());
        
        let cw = self.argument_of_periapsis.cos();
        let sw = self.argument_of_periapsis.sin();
        let co = self.longitude_of_ascending_node.cos();
        let so = self.longitude_of_ascending_node.sin();
        let ci = self.inclination.cos();
        let si = self.inclination.sin();
        let mat = Matrix3x2::new(
            cw * co - sw * ci * so, -sw * co - cw * ci * so,
            cw * so + sw * ci * co, cw * ci * co - sw * so,
            sw * si, cw * si,
        );

        let position = Point3::from(mat * orbit_pos);
        let velocity = mat * orbit_vel;

        StateVector { position, velocity }
    }
}

fn fix_quadrant(val: f64, sign: f64) -> f64 {
    if sign >= 0.0 {
        val
    } else {
        TAU - val
    }
}
