use prc::{hash40::Hash40, Prc};

#[derive(Debug, Prc, Clone)]
pub struct SwingPrc {
    pub swingbones: Vec<SwingBone>,
    pub spheres: Vec<Sphere>,
    pub ovals: Vec<Oval>,
    pub ellipsoids: Vec<Ellipsoid>,
    pub capsules: Vec<Capsule>,
    pub planes: Vec<Plane>,
}

#[derive(Debug, Prc, Clone)]
pub struct SwingBone {
    pub name: Hash40,
    pub start_bonename: Hash40,
    pub end_bonename: Hash40,
    pub params: Vec<Param>,
    pub isskirt: i8,
    pub rotateorder: i32, // TODO: can be i32 or i8
    pub curverotatex: i8,
    #[prc(hash = 0x0f7316a113)]
    pub unk: Option<i8>,
}

#[derive(Debug, Prc, Clone)]
pub struct Param {
    pub airresistance: f32,
    pub waterresistance: f32,
    pub minanglez: f32,
    pub maxanglez: f32,
    pub minangley: f32,
    pub maxangley: f32,
    pub collisionsizetip: f32,
    pub collisionsizeroot: f32,
    pub frictionrate: f32,
    pub goalstrength: f32,
    #[prc(hash = 0x0cc10e5d3a)]
    pub unk: f32,
    pub localgravity: f32,
    pub fallspeedscale: f32,
    pub groundhit: i8,
    pub windaffect: f32,
    pub collisions: Vec<Hash40>,
}

#[derive(Debug, Prc, Clone)]
pub struct Sphere {
    pub name: Hash40,
    pub bonename: Hash40,
    pub cx: f32,
    pub cy: f32,
    pub cz: f32,
    pub radius: f32,
}

#[derive(Debug, Prc, Clone)]
pub struct Oval {
    pub name: Hash40,
    pub start_bonename: Hash40,
    pub end_bonename: Hash40,
    pub radius: f32,
    pub start_offset_x: f32,
    pub start_offset_y: f32,
    pub start_offset_z: f32,
    pub end_offset_x: f32,
    pub end_offset_y: f32,
    pub end_offset_z: f32,
}

#[derive(Debug, Prc, Clone)]
pub struct Ellipsoid {
    pub name: Hash40,
    pub bonename: Hash40,
    pub cx: f32,
    pub cy: f32,
    pub cz: f32,
    pub rx: f32,
    pub ry: f32,
    pub rz: f32,
    pub sx: f32,
    pub sy: f32,
    pub sz: f32,
}

#[derive(Debug, Prc, Clone)]
pub struct Capsule {
    pub name: Hash40,
    pub start_bonename: Hash40,
    pub end_bonename: Hash40,
    pub start_offset_x: f32,
    pub start_offset_y: f32,
    pub start_offset_z: f32,
    pub end_offset_x: f32,
    pub end_offset_y: f32,
    pub end_offset_z: f32,
    pub start_radius: f32,
    pub end_radius: f32,
}

#[derive(Debug, Prc, Clone)]
pub struct Plane {
    pub name: Hash40,
    pub bonename: Hash40,
    pub nx: f32,
    pub ny: f32,
    pub nz: f32,
    pub distance: f32,
}

// TODO: Get data from swing.prc.
// spheres: cxyz, radius
// ovals:
// ellipsoids: cxyz, rxyz, sxyz
// capsules: start_offset_xyz, end_offset_xyz, start_radius, end_radius
// planes: nxyz, distance

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prc() {
        let mut reader = std::io::Cursor::new(std::fs::read("../swing.prc").unwrap());
        let prc = SwingPrc::read_file(&mut reader).unwrap();
        dbg!(prc);
    }
}
