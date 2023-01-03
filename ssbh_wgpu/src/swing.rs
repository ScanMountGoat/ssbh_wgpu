use prc::{
    hash40::Hash40,
    prc_trait::{ErrorKind, FileOffsets},
    Prc,
};
use std::{io::SeekFrom, path::Path};

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
    pub rotateorder: RotateOrder,
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

// Some files use slightly different field types
#[derive(Debug, Clone)]
pub enum RotateOrder {
    // ex: /fighter/sonic/motion/body/c00/swing.prc
    I8(i8),
    // ex:/fighter/bayonetta/motion/body/c00/swing.prc
    I32(i32),
}

impl Prc for RotateOrder {
    fn read_param<R: std::io::Read + std::io::Seek>(
        reader: &mut R,
        offsets: FileOffsets,
    ) -> prc::prc_trait::Result<Self> {
        let param_start = reader.stream_position().map_err(prc_io_error)?;

        <i32 as Prc>::read_param(reader, offsets)
            .map(RotateOrder::I32)
            .or_else(|err| {
                // Put the reader at the start of the param again.
                reader
                    .seek(SeekFrom::Start(param_start))
                    .map_err(prc_io_error)?;

                if matches!(&err.kind, ErrorKind::WrongParamNumber { .. }) {
                    <i8 as Prc>::read_param(reader, offsets).map(RotateOrder::I8)
                } else {
                    Err(err)
                }
            })
    }
}

fn prc_io_error(e: std::io::Error) -> prc::prc_trait::Error {
    prc::prc_trait::Error {
        path: Vec::new(),
        position: Ok(0),
        kind: prc::prc_trait::ErrorKind::Io(e),
    }
}

impl SwingPrc {
    // TODO: Replace with Result once prc-rs has better error types.
    pub fn from_file<P: AsRef<Path>>(path: P) -> Option<Self> {
        let mut reader = std::io::BufReader::new(std::fs::File::open(path).ok()?);
        SwingPrc::read_file(&mut reader).ok()
    }
}
