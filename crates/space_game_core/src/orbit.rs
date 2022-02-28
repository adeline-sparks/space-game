use std::f64::consts::{PI, TAU};

use nalgebra::Vector3;

#[derive(Clone, Debug)]
pub struct OrbitalElements {
    pub semi_major_axis: f64,
    pub eccentricity: f64,
    pub inclination: f64,
    pub longitude_of_ascending_node: f64,
    pub argument_of_periapsis: f64,
    pub mean_anomaly: f64,
}

#[derive(Clone, Debug)]
pub struct StateVector {
    pub position: Vector3<f64>,
    pub velocity: Vector3<f64>,
}

pub const GRAVITATIONAL_CONSTANT: f64 = 6.6743015e-11;
pub const EPSILON: f64 = 1e-11;

impl OrbitalElements {
    pub fn from_state_vector(sv: &StateVector, central_body_mass: f64) -> Self {
        let grav = GRAVITATIONAL_CONSTANT * central_body_mass;
        if grav == 0.0 {
            todo!()
        }

        let momentum_vec = sv.position.cross(&sv.velocity);
        let momentum = momentum_vec.magnitude();
        if momentum == 0.0 {
            todo!();
        }

        let node_vec = Vector3::z().cross(&momentum_vec);
        let node = node_vec.magnitude();

        let position_mag = sv.position.magnitude();
        if position_mag == 0.0 {
            todo!()
        }

        let velocity_mag = sv.velocity.magnitude();

        let eccentricity_vec = (velocity_mag * velocity_mag / grav - 1.0 / position_mag)
            * sv.position
            - ((sv.position.dot(&sv.velocity) / grav) * sv.velocity);
        let eccentricity = eccentricity_vec.magnitude();
        if (1.0 - eccentricity).abs() <= 1e-6 {
            todo!()
        }

        let energy = 0.5 * velocity_mag * velocity_mag - grav / position_mag;
        if energy == 0.0 {
            todo!()
        }

        let semi_major_axis = -grav / (2.0 * energy);
        let inclination = (momentum_vec.z / momentum).acos();
        let inclination_zero = inclination <= 1e-11;
        let inclination_pi = inclination >= PI - 1e-11;

        let equatorial = inclination_zero || inclination_pi;

        let longitude_of_ascending_node = if equatorial {
            0.0
        } else {
            let result = (node_vec.x / node).acos();
            if node_vec.y < 0.0 {
                TAU - result
            } else {
                result
            }
        };

        let circular = eccentricity <= 1e-11;

        let argument_of_periapsis = match (circular, equatorial) {
            (true, _) => 0.0,
            (false, false) => {
                let result = (node_vec.dot(&eccentricity_vec) / (node * eccentricity)).acos();
                if eccentricity_vec.z < 0.0 {
                    TAU - result
                } else {
                    result
                }
            }
            (false, true) => {
                let result = (eccentricity_vec.x / eccentricity).acos();
                if eccentricity_vec.y < 0.0 {
                    TAU - result
                } else {
                    result
                }
            }
        };

        let true_anomaly = match (circular, equatorial) {
            (false, _) => {
                let result =
                    (eccentricity_vec.dot(&sv.position) / (eccentricity * position_mag)).acos();
                if sv.position.dot(&sv.velocity) < 0.0 {
                    TAU - result
                } else {
                    result
                }
            }
            (true, false) => {
                let result = (node_vec.dot(&sv.position) / (node * position_mag)).acos();
                if sv.position.z < 0.0 {
                    TAU - result
                } else {
                    result
                }
            }
            (true, true) => {
                let result = sv.position.y.atan2(sv.position.x);
                if inclination_pi {
                    TAU - result
                } else {
                    result
                }
            }
        };

        let tol = 1e-3;
        let mean_anomaly = if eccentricity < (1.0 - tol) {
            let cos_ta = true_anomaly.cos();
            let ecc_cos_ta = eccentricity * cos_ta;
            let sin_ea = ((1.0 - eccentricity * eccentricity).sqrt() * true_anomaly.sin())
                / (1.0 + ecc_cos_ta);
            let cos_ea = (eccentricity + cos_ta) / (1.0 + ecc_cos_ta);
            let eccentric_anomaly = sin_ea.atan2(cos_ea);
            let result = eccentric_anomaly - eccentricity * eccentric_anomaly.sin();
            if result < 0.0 {
                TAU + result
            } else {
                result
            }
        } else if eccentricity > (1.0 + tol) {
            let tanh_ha2 =
                (true_anomaly / 2.0).tan() * ((eccentricity - 1.0) / (eccentricity + 1.0)).sqrt();
            let hyperbolic_anomaly = 2.0 * tanh_ha2.atanh();
            eccentricity * hyperbolic_anomaly.sinh() - hyperbolic_anomaly
        } else {
            todo!();
        };

        OrbitalElements {
            semi_major_axis,
            eccentricity,
            inclination,
            longitude_of_ascending_node,
            argument_of_periapsis,
            mean_anomaly,
        }
    }

    pub fn true_anomaly(&self) -> f64 {
        if self.eccentricity <= 1.0 {
            let mut e2 = self.mean_anomaly + self.eccentricity * self.mean_anomaly.sin();
            let result = loop {
                let temp = 1.0 - self.eccentricity * e2.cos();
                if temp.abs() < 1e-30 {
                    todo!();
                }
                let e1 = e2 - (e2 - self.eccentricity * e2.sin() - self.mean_anomaly) / temp;
                if (e2 - e1).abs() < 1e-8 {
                    break e1;
                }
                e2 = e1;
            };
            let eccentric_anomaly = if result < 0.0 { TAU + result } else { result };

            let result = if (eccentric_anomaly - PI).abs() >= 1e-8 {
                let temp = 1.0 - self.eccentricity;
                if temp.abs() < 1e-30 {
                    todo!();
                }
                let temp2 = (1.0 + self.eccentricity) / temp;
                if temp2 < 0.0 {
                    todo!();
                }
                2.0 * (temp2.sqrt() * (self.eccentricity / 2.0).tan()).atan()
            } else {
                eccentric_anomaly
            };

            if result < 0.0 {
                result + TAU
            } else {
                result
            }
        } else {
            let mut f2 = 0.0f64;
            let hyperbolic_anomaly = loop {
                let temp = self.eccentricity * f2.cosh() - 1.0;
                if temp.abs() < 1e-30 {
                    todo!();
                }
                let f1 = f2 - (self.eccentricity * f2.sinh() - f2 - self.mean_anomaly) / temp;
                if (f2 - f1).abs() < 1e-8 {
                    break f1;
                }
                f2 = f1;
            };

            let temp = self.eccentricity - 1.0;
            if temp.abs() < 1e-30 {
                todo!();
            }
            let temp2 = (self.eccentricity + 1.0) / temp;
            if temp2 < 0.0 {
                todo!();
            }

            let result = 2.0 * (temp2.sqrt() * (hyperbolic_anomaly / 2.0).tanh()).atan();
            if result < 0.0 {
                result + TAU
            } else {
                result
            }
        }
    }

    pub fn as_state_vector(&self, central_body_mass: f64) -> StateVector {
        let grav = GRAVITATIONAL_CONSTANT * central_body_mass;

        let true_anomaly = self.true_anomaly();
        let (sin_anom, cos_anom) = true_anomaly.sin_cos();

        let p = self.semi_major_axis * (1.0 - self.eccentricity * self.eccentricity);
        let rad = p / (1.0 + self.eccentricity * cos_anom);
        let sqrt_grav_p = (grav / p).sqrt();

        let (sin_inc, cos_inc) = self.inclination.sin_cos();
        let (sin_long, cos_long) = self.longitude_of_ascending_node.sin_cos();
        let (sin_per, cos_per) = self.argument_of_periapsis.sin_cos();
        let cos_anom_plus_e = cos_anom + self.eccentricity;
        let (sin_per_anom, cos_per_anom) = (self.argument_of_periapsis + true_anomaly).sin_cos();

        let x = rad * (cos_per_anom * cos_long - cos_inc * sin_per_anom * sin_long);
        let y = rad * (cos_per_anom * sin_long + cos_inc * sin_per_anom * cos_long);
        let z = rad * sin_per_anom * sin_inc;

        let vx =
            sqrt_grav_p * cos_anom_plus_e * (-sin_per * cos_long - cos_inc * sin_long * cos_per)
                - sqrt_grav_p * sin_anom * (cos_per * cos_long - cos_inc * sin_long * sin_per);
        let vy =
            sqrt_grav_p * cos_anom_plus_e * (-sin_per * sin_long + cos_inc * cos_long * cos_per)
                - sqrt_grav_p * sin_anom * (cos_per * cos_long + cos_inc * sin_long * sin_per);
        let vz = sqrt_grav_p * (cos_anom_plus_e * sin_inc * cos_per - sin_anom * sin_inc * sin_per);

        StateVector {
            position: Vector3::new(x, y, z),
            velocity: Vector3::new(vx, vy, vz),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EARTH_MASS: f64 = 5.972e24;
    const EARTH_RADIUS: f64 = 6378e3;

    #[test]
    fn orbital_elements() {
        let apogee = 200e3;
        let vel = 7.79e3;
        let sv = StateVector {
            position: Vector3::new(EARTH_RADIUS + apogee, 0.0, 0.0),
            velocity: Vector3::new(0.0, vel, 0.0),
        };
        dbg!(&sv);
        let oe = OrbitalElements::from_state_vector(&sv, EARTH_MASS);
        dbg!(&oe);
        let sv2 = oe.as_state_vector(EARTH_MASS);
        dbg!(&sv2);
        let pos_error = (sv.position - sv2.position).norm();
        dbg!(&pos_error);
        let vel_error = (sv.velocity - sv2.velocity).norm();
        dbg!(&vel_error);
        assert!(pos_error < 1.0 && vel_error < 1.0);
    }
}
