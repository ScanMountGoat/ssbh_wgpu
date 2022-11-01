use prc::{hash40::Hash40, Prc};

#[derive(Debug, Prc)]
pub struct SwingPrc {
    pub swingbones: Vec<SwingBone>,
    pub spheres: Vec<Sphere>,
}

#[derive(Debug, Prc)]
pub struct SwingBone {
    pub name: Hash40,
    pub start_bonename: Hash40,
    pub end_bonename: Hash40,
    pub params: Vec<Param>,
    pub isskirt: i8,
    pub rotateorder: i32,
    pub curverotatex: i8,
    #[prc(hash = 0x0f7316a113)]
    pub unk: i8,
}

#[derive(Debug, Prc)]

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

#[derive(Debug, Prc)]
pub struct Sphere {
    pub name: Hash40,
    pub bonename: Hash40,
    pub cx: f32,
    pub cy: f32,
    pub cz: f32,
    pub radius: f32,
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
