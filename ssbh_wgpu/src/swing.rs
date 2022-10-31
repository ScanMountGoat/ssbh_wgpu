pub struct Sphere {
    pub bone_name: String,
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
