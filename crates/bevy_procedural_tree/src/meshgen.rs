use core::f32;
use std::f32::consts::PI;

use bevy::mesh::Indices;
use bevy::mesh::PrimitiveTopology;
use bevy::{asset::RenderAssetUsages, prelude::*};
use fastrand::Rng;

use crate::{enums::TreeType, settings::TreeMeshSettings};
use crate::errors::IndicesOverflowError;

#[derive(Debug, Clone)]
struct BranchGenState {
    pub origin: Vec3,
    pub orientation: Quat,
    pub length: f32,
    pub start_radius: f32,
    pub taper: f32,
    pub twist: f32,
    pub gnarliness: f32,
    pub level: usize,
    pub recursion_count: usize,
    pub sections: usize,
    pub segments: usize,
}

#[derive(Debug, Clone)]
struct SectionData {
    pub origin: Vec3,
    pub orientation: Quat,
    pub radius: f32
}

#[cfg(not(feature = "u32_indices"))]
#[derive(Debug, Default)]
struct MeshAttributes {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    indices: Vec<u16>
}

#[cfg(feature = "u32_indices")]
#[derive(Debug, Default)]
struct MeshAttributes {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    indices: Vec<u32>
}

/// Generate two meshes: the trunk/branches and the leaves
/// 
/// Both meshes together represent a tree. The mesh is built according to the provided TreeMeshSettings.
/// If the tree should be reproduced use the same settings and the same Rng (including the same seed).
pub fn generate_tree_meshes(settings: &TreeMeshSettings, rng: &mut Rng) -> Result<(Mesh, Mesh), BevyError> { 
    let state: BranchGenState = BranchGenState {
        origin: Vec3::ZERO,
        orientation: Quat::IDENTITY,
        length: settings.branch.length[0],
        start_radius: settings.branch.trunk_base_radius,
        taper: settings.branch.taper[0],
        twist: settings.branch.twist[0],
        gnarliness: settings.branch.gnarliness[0],
        level: 0,
        recursion_count: 0,
        sections: settings.branch.sections[0] as usize,
        segments: settings.branch.segments[0] as usize,
    };
    generate_branches_internal(settings, state, rng)
}

fn generate_branches_internal(settings: &TreeMeshSettings, state: BranchGenState, rng: &mut Rng) -> Result<(Mesh, Mesh), BevyError> { 
    // Allocate mesh attributes
    // TODO allocate just enough to reduce reallocations
    let mut branches_attributes: MeshAttributes = MeshAttributes::default();
    let mut leaves_attributes: MeshAttributes = MeshAttributes::default();
    //let mut branches_colors:    Vec<[f32; 4]> = Vec::new(); //with_capacity(rings * ring_stride);

    recurse_a_branch(settings, state, rng, &mut branches_attributes, &mut leaves_attributes)?;
    
    // build meshes
    let mut branches_mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD);
    branches_mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, branches_attributes.positions);
    branches_mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, branches_attributes.normals);
    branches_mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, branches_attributes.uvs);
    #[cfg(not(feature = "u32_indices"))]
    branches_mesh.insert_indices(Indices::U16(branches_attributes.indices));
    #[cfg(feature = "u32_indices")]
    branches_mesh.insert_indices(Indices::U32(branches_attributes.indices));
    //branches_mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, branches_colors);
    branches_mesh.generate_tangents()?;

    let mut leaves_mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD);
    leaves_mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, leaves_attributes.positions);
    leaves_mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, leaves_attributes.normals);
    leaves_mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, leaves_attributes.uvs);
    #[cfg(not(feature = "u32_indices"))]
    leaves_mesh.insert_indices(Indices::U16(leaves_attributes.indices));
    #[cfg(feature = "u32_indices")]
    leaves_mesh.insert_indices(Indices::U32(leaves_attributes.indices));
    leaves_mesh.generate_tangents()?;

    Ok((branches_mesh, leaves_mesh))
}

#[allow(clippy::too_many_arguments)]
fn recurse_a_branch(
    settings: &TreeMeshSettings,
    state: BranchGenState,
    rng: &mut Rng,
    branches_attributes: &mut MeshAttributes,
    //branches_colors: &mut Vec<[f32; 4]>,
    leaves_attributes: &mut MeshAttributes
) -> Result<(), BevyError>
{       
    #[cfg(not(feature = "u32_indices"))]
    let indices_start: u16 = branches_attributes.positions.len() as u16;
    #[cfg(feature = "u32_indices")]
    let indices_start: u32 = branches_attributes.positions.len() as u32;
    // catch going outside of the allowed range early and tell the user
    let approx_amount_of_indices_of_this_branch: usize = state.sections * state.segments * 6;
    #[cfg(not(feature = "u32_indices"))]
    if branches_attributes.indices.len() >= (u16::MAX as usize - approx_amount_of_indices_of_this_branch) {
        return Err(IndicesOverflowError.into());
    }
    #[cfg(feature = "u32_indices")]
    if branches_attributes.indices.len() >= (u32::MAX as usize - approx_amount_of_indices_of_this_branch) {
        return Err(IndicesOverflowError.into());
    }

    // local section storage    
    let mut sections: Vec<SectionData> = Vec::with_capacity(state.sections);
    
    // calculate the length of each section (one vertical ring)
        
    // give the different parts of a Deciduous branch a different length based on the level (lower level = more length)
    // the sum should be equal to the target length (state.length for Deciduous trunks; at level 0)
    // target formula: (max_level - current_level + 1) / sum of (possible_levels+1)
    let target_pieces: f32 = (1..=(settings.branch.levels as usize+1)).sum::<usize>() as f32;
    let factor_for_length: f32 = if state.level > 0 {1.0} else {
        match settings.tree_type {
            TreeType::Deciduous => (settings.branch.levels as usize - state.recursion_count + 1) as f32 / target_pieces,
            TreeType::Evergreen => 1.0,
        }
    };

    let section_length = state.length / state.sections as f32 * factor_for_length;

    // create state for iterations
    let mut section_orientation = state.orientation;
    let mut section_origin = state.origin;
    let mut section_radius = state.start_radius;

    // precalculate force quat (to not do it in the loop)
    // TODO would be even faster if done one level higher (i.e. in the state)
    let branch_force_dir = if settings.branch.force.direction.length_squared() >= f32::EPSILON {
        settings.branch.force.direction.normalize()
    } else {
        Vec3::Y
    };
    let branch_force_quat = Quat::from_rotation_arc(Vec3::Y, branch_force_dir);

    // amount of taper to add per each section to reach target taper; each level may have a different target taper
    // for Evergreen we need 'sections' steps, so that at the top we have the target taper
    // for Deciduous we need even more steps, due to the trunk being build from sections*levels parts
    let taper_amount_per_section = match settings.tree_type {
        TreeType::Deciduous => f32::powf(1.0 - state.taper.clamp(0.0, 0.9999), (1.0/state.sections as f32) / (f32::from(settings.branch.levels) + 1.0)),
        TreeType::Evergreen => f32::powf(1.0 - state.taper.clamp(0.0, 0.9999), 1.0/state.sections as f32), 
    };
    
    // iterate over sections + one final ring
    // the =sections is needed because to have x sections, we need x+1 rows of vertices
    for section_counter in 0..=state.sections {
        // update radius
        if section_counter == state.sections && !((state.level == 0) && matches!(settings.tree_type, TreeType::Deciduous)) {
            // last ring of the last section is a tip (except the main branch/trunk of deciduous trees)
            section_radius = f32::EPSILON;
        } 
    
        // save the first vertex to create a ring in the end
        let mut first_pos = Vec3::ZERO;
        let mut first_nrm = Vec3::ZERO;
        let mut first_v  = 0.0;
    
        // for each segment create a single vertex
        for segment_counter in 0..state.segments {
            let angle = (2.0 * PI * segment_counter as f32) / state.segments as f32;
            let (sin, cos) = angle.sin_cos();    
            
            let local_pos = Vec3::new(cos * section_radius, 0.0, sin * section_radius);
            let local_normal = Vec3::new(cos, 0.0, sin);
    
            let vertex = (section_orientation * local_pos) + section_origin;
            let normal = (section_orientation * local_normal).normalize();
    
            let u = segment_counter as f32 / state.segments as f32;
            let v = if section_counter % 2 == 0 { 0.0 } else { 1.0 };
        
            if segment_counter == 0 {
                first_pos = vertex;
                first_nrm = normal;
                first_v = v;
            }            
            branches_attributes.positions.push(vertex.to_array());
            branches_attributes.normals.push(normal.to_array());
            branches_attributes.uvs.push([u,v]);
            // color code levels for debugging
            // match BranchRecursionLevel::try_from(state.recursion_count as u8).unwrap() {
            //     BranchRecursionLevel::Zero => branches_colors.push([1.0, 0.0, 0.0, 1.0]),
            //     BranchRecursionLevel::One => branches_colors.push([0.0, 1.0, 0.0, 1.0]),
            //     BranchRecursionLevel::Two => branches_colors.push([0.0, 0.0, 1.0, 1.0]),
            //     BranchRecursionLevel::Three => branches_colors.push([0.0, 1.0, 1.0, 1.0]),
            //     //BranchRecursionLevel::Four => colors.push([1.0, 1.0, 1.0, 1.0]),
            // }
            
        } // END for each segment
    
        // duplicate of the first vertex to create a full ring (with different uv)
        branches_attributes.positions.push(first_pos.to_array());
        branches_attributes.normals.push(first_nrm.to_array());
        branches_attributes.uvs.push([1.0, first_v]);
        // color code levels for debugging
        // match BranchRecursionLevel::try_from(state.recursion_count as u8).unwrap() {
        //     BranchRecursionLevel::Zero => branches_colors.push([1.0, 0.0, 0.0, 1.0]),
        //     BranchRecursionLevel::One => branches_colors.push([0.0, 1.0, 0.0, 1.0]),
        //     BranchRecursionLevel::Two => branches_colors.push([0.0, 0.0, 1.0, 1.0]),
        //     BranchRecursionLevel::Three => branches_colors.push([0.0, 1.0, 1.0, 1.0]),
        //     //BranchRecursionLevel::Four => colors.push([1.0, 1.0, 1.0, 1.0]),
        // }
    
        // save section data for later allow branches to grow from them
        sections.push(SectionData {
            origin: section_origin,
            orientation: section_orientation,
            radius: section_radius
        });
    
        //
        // Update section parameters for next section
        //
        if section_counter < state.sections {
            // Gnarliness: random tilt around x and z
            let gn = state.gnarliness * 0.4 / section_radius.sqrt(); // 0.4 chosen by trial and error to look natural (values between 0..1 make the most sense now; larger is still possible)
            let dx = (rng.f32() - 0.5) * gn;
            let dz = (rng.f32() - 0.5) * gn;
            let q_gnarl = Quat::from_euler(EulerRot::XYZ, dx, 0.0, dz);

            // twist around y-axis
            let q_twist = Quat::from_axis_angle(Vec3::Y, state.twist);

            // apply gnarl and twist
            section_orientation = (q_gnarl * section_orientation) * q_twist;

            // slerp the target orientation in the direction of the branch.force based on the given strength and radius of the branch
            let radius_factor = 1.0 - (section_radius / settings.branch.force.radius_cutoff).clamp(0.0, 1.0);
            let strength_per_radius = (settings.branch.force.strength * radius_factor / 2.0).clamp(0.0, 1.0); // 2 chosen by trial and error to look natural (values between 0..1 make the most sense now; larger is still possible)
            section_orientation = section_orientation.slerp(branch_force_quat, strength_per_radius);

            // taper
            section_radius *= taper_amount_per_section;

            // direction (go along the branch)
            let up = section_orientation * Vec3::Y;
            section_origin += up * section_length;
        }        
    } // END for each section
    
    // Indices (triangles) are build around the ring per segment
    #[cfg(not(feature = "u32_indices"))]
    {
        let ring_stride: u16 = state.segments as u16 + 1;    
        for i in 0..state.sections as u16 {
            for j in 0..state.segments as u16 {
                let a: u16 = i * ring_stride        + j         + indices_start;
                let b: u16 = i * ring_stride        + (j + 1)   + indices_start;
                let c: u16 = a + ring_stride;
                let d: u16 = b + ring_stride;
        
                branches_attributes.indices.extend_from_slice(&[a, c, b, b, c, d]);
            }
        }
    }  
    #[cfg(feature = "u32_indices")]
    {
        let ring_stride: u32 = state.segments as u32 + 1;    
        for i in 0..state.sections as u32 {
            for j in 0..state.segments as u32 {
                let a: u32 = i * ring_stride        + j         + indices_start;
                let b: u32 = i * ring_stride        + (j + 1)   + indices_start;
                let c: u32 = a + ring_stride;
                let d: u32 = b + ring_stride;
        
                branches_attributes.indices.extend_from_slice(&[a, c, b, b, c, d]);
            }
        }
    }  


    if matches!(settings.tree_type, TreeType::Deciduous) && state.level == 0 {
        if state.recursion_count < settings.branch.levels as usize {
            // Deciduous trunks are build itnernally from multiple continous branches (for nicer branch generation)
            let additional_trunk_part = BranchGenState {
                origin: section_origin,
                orientation: section_orientation,
                length: state.length, 
                start_radius: section_radius,
                taper: state.taper,
                twist: state.twist,
                gnarliness: state.gnarliness,
                level: state.level,
                recursion_count: state.recursion_count + 1,
                // Section count and segment count must be same as parent branch
                // since the child branch is growing from the end of the parent branch           
                sections: state.sections,
                segments: state.segments,
            };
            recurse_a_branch(settings, additional_trunk_part, rng, branches_attributes, leaves_attributes)?;
        }
        else {
            // generate a nice single leaf at the top
            generate_leaf(settings, section_origin, section_orientation, rng, leaves_attributes)?;
        }
    }

    if state.recursion_count == settings.branch.levels as usize {
        // generate leaves at the different sections of this branch
        // state.level is constant in this case, we keep it as a parameter for possible future functionality
        generate_leaves(&sections, settings, rng, leaves_attributes)?;
    }
    else {
        for child_branch_state in generate_child_branches(
            settings.branch.children[state.recursion_count],
            state.recursion_count + 1,
            &sections,
            settings,
            rng
        ) {
            recurse_a_branch(settings, child_branch_state, rng, branches_attributes, leaves_attributes)?;
        }
    }

    Ok(())
}



fn generate_child_branches (
    count: u8,
    level: usize,
    parent_sections: &[SectionData],
    settings: &TreeMeshSettings,
    rng: &mut Rng,
) -> Vec<BranchGenState> {
    if count == 0 || parent_sections.is_empty(){
        return Vec::new();
    }

    let radial_offset: f32 = rng.f32(); 
    let section_count_minus_one: usize = parent_sections.len().saturating_sub(1);  

    let mut out = Vec::with_capacity(count as usize);
    for i in 0..count {
        // lowest start position along the parent branch as a factor
        let child_start_factor = f32::lerp(settings.branch.start[level].clamp(0.0, 1.0), 1.0, rng.f32());

        // calculate a factor between two sections based on the possible range
        let child_branch_pos = child_start_factor * section_count_minus_one as f32;
        let section_index = child_branch_pos.floor() as usize;
        let branch_height_factor = (child_branch_pos - section_index as f32).clamp(0.0, 1.0);

        // calculate target sections where to place the branch
        let section_a_index = section_index; 
        let section_b_index = (section_index + 1).min(section_count_minus_one);
        let section_a = &parent_sections[section_a_index];
        let section_b = &parent_sections[section_b_index];

        // interpolate the placement between section a and b
        let child_branch_origin = section_a.origin.lerp(section_b.origin, branch_height_factor);

        // calculate radius
        // TODO is this correct?
        let radius_setting = settings.branch.radius_factor[level];
        let parent_radius = f32::lerp(section_a.radius, section_b.radius, branch_height_factor);
        let child_branch_radius = radius_setting * parent_radius;

        // orient along the parent sections
        let parent_orientation = section_b.orientation.slerp(section_a.orientation, branch_height_factor);

        // calculate needed angles 
        let radial_angle = 2.0 * std::f32::consts::PI * (radial_offset + (i as f32) / (count as f32));
        let angle_rad = settings.branch.angle[level].to_radians();
        let q1 = Quat::from_axis_angle(Vec3::X, angle_rad);
        let q2 = Quat::from_axis_angle(Vec3::Y, radial_angle);
        let child_quat = parent_orientation * q2 * q1;

        // target length
        let mut child_len = settings.branch.length[level];
        if settings.tree_type == TreeType::Evergreen {
            child_len *= 1.0 - child_start_factor;
        }

        out.push(BranchGenState {
            origin: child_branch_origin,
            orientation: child_quat,
            length: child_len,
            start_radius: child_branch_radius,
            level,
            recursion_count: level,
            taper: settings.branch.taper[level],
            twist: settings.branch.twist[level],
            gnarliness: settings.branch.gnarliness[level],
            sections: settings.branch.sections[level].into(),
            segments: settings.branch.segments[level].into()
        });
    }

    out
}

fn generate_leaves(
    sections: &[SectionData],
    settings: &TreeMeshSettings,
    rng: &mut Rng,
    leaves_attributes: &mut MeshAttributes
) -> Result<(), BevyError>
{
    // catch going outside of the allowed range early and tell the user
    let approx_amount_of_indices_of_this_leaf: usize = settings.leaves.count as usize * 6;
    #[cfg(not(feature = "u32_indices"))]
    if leaves_attributes.indices.len() >= (u16::MAX as usize - approx_amount_of_indices_of_this_leaf) {
        return Err(IndicesOverflowError.into());
    }
    #[cfg(feature = "u32_indices")]
    if leaves_attributes.indices.len() >= (u32::MAX as usize - approx_amount_of_indices_of_this_leaf) {
        return Err(IndicesOverflowError.into());
    }

    let radial_offset: f32 = rng.f32();
    let section_count_minus_one: usize = sections.len().saturating_sub(1);  

    for i in 0..settings.leaves.count {
        // how far along the section should this leaf start
        let leaf_start = f32::lerp(settings.leaves.start.clamp(0.0, 1.0), 1.0, rng.f32());

        // find relevant sections depending on leaf_start
        let leaf_pos = leaf_start * section_count_minus_one as f32;
        let section_index = leaf_pos.floor() as usize;
        let leaf_height_factor = (leaf_pos - section_index as f32).clamp(0.0, 1.0);

        // calculate target sections where to place the leaf
        let section_a_index = section_index; 
        let section_b_index = (section_index + 1).min(section_count_minus_one);
        let section_a = &sections[section_a_index];
        let section_b = &sections[section_b_index];

        // interpolate the placement between section a and b
        let leaf_origin = section_a.origin.lerp(section_b.origin, leaf_height_factor);

        // interpolate the orientation; orient along the parent sections
        let parent_orientation = section_b.orientation.slerp(section_a.orientation, leaf_height_factor);

        // calculate needed angles 
        let radial_angle = 2.0 * std::f32::consts::PI * (radial_offset + (i as f32) / (settings.leaves.count as f32));
        let angle_rad = settings.leaves.angle.to_radians();
        let q1 = Quat::from_axis_angle(Vec3::X, angle_rad);
        let q2 = Quat::from_axis_angle(Vec3::Y, radial_angle);
        let child_quat = parent_orientation * q2 * q1;

        generate_leaf(settings, leaf_origin, child_quat, rng, leaves_attributes)?;
    }

    Ok(())
}

fn generate_leaf(
    settings: &TreeMeshSettings,
    origin: Vec3,
    orientation: Quat,
    rng: &mut Rng,
    leaves_attributes: &mut MeshAttributes
) -> Result<(), BevyError>
{
    #[cfg(not(feature = "u32_indices"))]
    let mut indices_start: u16 = leaves_attributes.positions.len() as u16;
    #[cfg(feature = "u32_indices")]
    let mut indices_start: u32 = leaves_attributes.positions.len() as u32;

    let leaf_size_variance = (2.0 * rng.f32() - 1.0) * settings.leaves.size_variance.max(0.0);
    let leaf_size = settings.leaves.size * (1.0 + leaf_size_variance);
    let leaf_size_half = leaf_size / 2.0;

    let rotations: &[f32] = match settings.leaves.leaf_billboard {
        crate::enums::LeafBillboard::Single => &[0.0],
        crate::enums::LeafBillboard::Double => &[0.0, f32::consts::FRAC_PI_2],
    };

    for rotation in rotations.iter() {
        let leaf_orientation = orientation * Quat::from_euler(EulerRot::XYX, 0.0, *rotation, 0.0);

        // vertice positions
        let vertices: Vec<[f32;3]> = [
        Vec3::new(-leaf_size_half, leaf_size, 0.0),
        Vec3::new(-leaf_size_half, 0.0, 0.0),
        Vec3::new(leaf_size_half, 0.0, 0.0),
        Vec3::new(leaf_size_half, leaf_size, 0.0),
        ].into_iter().map(|v| (leaf_orientation * v + origin).to_array()).collect();

        leaves_attributes.positions.extend_from_slice(&vertices);

        // vertice normals
        let normal: [f32;3] = (leaf_orientation * Vec3::new(0.0, 0.0, 1.0)).to_array();

        leaves_attributes.normals.push(normal);
        leaves_attributes.normals.push(normal);
        leaves_attributes.normals.push(normal);
        leaves_attributes.normals.push(normal);

        // uvs and indices
        leaves_attributes.uvs.extend_from_slice(&[[0.0, 0.0],[0.0, 1.0],[1.0, 1.0],[1.0, 0.0]]);
        leaves_attributes.indices.extend_from_slice(&[indices_start, indices_start+1, indices_start+2, indices_start, indices_start+2, indices_start+3]);
        indices_start += 4;
    }

    Ok(())
}