//! USDA (text format) parser.
//!
//! Implements a proper recursive descent parser for USDA text,
//! supporting nested prims, properties, metadata, and relationships.

mod parser;
mod tokenizer;

use super::scene::*;
use super::UsdResult;

/// Parse USDA text content into a UsdStage.
pub fn parse(content: &str) -> UsdResult<UsdStage> {
    let tokens = tokenizer::tokenize(content);
    parser::parse_stage(&tokens, content)
}

#[cfg(test)]
mod tests {
    use super::super::scene::{NodeData, UpAxis};
    use super::parse;

    #[test]
    fn parse_stage_metadata() {
        let src = "#usda 1.0\n(\n    metersPerUnit = 0.01\n    upAxis = \"Z\"\n    timeCodesPerSecond = 30\n)\n";
        let stage = parse(src).unwrap();
        assert_eq!(stage.up_axis, UpAxis::ZUp);
        assert!((stage.meters_per_unit - 0.01).abs() < 1e-6);
        assert!((stage.time_codes_per_second - 30.0).abs() < 1e-6);
    }

    #[test]
    fn parse_default_metadata_when_absent() {
        let stage = parse("#usda 1.0\n").unwrap();
        // Defaults set by parse_stage.
        assert!((stage.meters_per_unit - 0.01).abs() < 1e-6);
        assert!((stage.time_codes_per_second - 24.0).abs() < 1e-6);
        assert_eq!(stage.up_axis, UpAxis::YUp); // scene default
    }

    #[test]
    fn parse_mesh_geometry() {
        let src = r#"#usda 1.0
def Mesh "Tri"
{
    point3f[] points = [(0, 0, 0), (1, 0, 0), (0, 1, 0)]
    int[] faceVertexCounts = [3]
    int[] faceVertexIndices = [0, 1, 2]
    normal3f[] normals = [(0, 0, 1), (0, 0, 1), (0, 0, 1)]
}
"#;
        let stage = parse(src).unwrap();
        assert_eq!(stage.meshes.len(), 1);
        let m = &stage.meshes[0];
        assert_eq!(m.name, "Tri");
        assert_eq!(m.path, "/Tri");
        assert_eq!(
            m.positions,
            vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]
        );
        assert_eq!(m.face_vertex_counts, vec![3]);
        assert_eq!(m.face_vertex_indices, vec![0, 1, 2]);
        assert_eq!(
            m.normals,
            vec![[0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0]]
        );

        // The scene root should hold one child node referencing mesh 0.
        assert_eq!(stage.root.children.len(), 1);
        assert!(matches!(stage.root.children[0].data, NodeData::Mesh(0)));
    }

    #[test]
    fn parse_mesh_uv_set() {
        let src = r#"#usda 1.0
def Mesh "M"
{
    point3f[] points = [(0, 0, 0)]
    texCoord2f[] primvars:st = [(0.25, 0.75)]
}
"#;
        let stage = parse(src).unwrap();
        let m = &stage.meshes[0];
        let st = m.uv_sets.get("st").expect("st uv set present");
        assert_eq!(st, &vec![[0.25, 0.75]]);
    }

    #[test]
    fn parse_material_and_binding_resolves() {
        let src = r#"#usda 1.0
def Material "Red"
{
    color3f inputs:diffuseColor = (1, 0, 0)
    float inputs:metallic = 0.5
    float inputs:roughness = 0.2
}

def Mesh "M"
{
    point3f[] points = [(0, 0, 0)]
    rel material:binding = </Red>
}
"#;
        let stage = parse(src).unwrap();
        assert_eq!(stage.materials.len(), 1);
        let mat = &stage.materials[0];
        assert_eq!(mat.name, "Red");
        assert_eq!(mat.diffuse_color, [1.0, 0.0, 0.0]);
        assert!((mat.metallic - 0.5).abs() < 1e-6);
        assert!((mat.roughness - 0.2).abs() < 1e-6);

        // Binding path </Red> resolves to material index 0.
        let mesh = &stage.meshes[0];
        assert_eq!(mesh.material_binding.as_deref(), Some("/Red"));
        assert_eq!(mesh.material_index, Some(0));
    }

    #[test]
    fn parse_nested_prims_build_paths() {
        let src = r#"#usda 1.0
def Xform "Root"
{
    def Mesh "Child"
    {
        point3f[] points = [(0, 0, 0)]
    }
}
"#;
        let stage = parse(src).unwrap();
        assert_eq!(stage.root.children.len(), 1);
        let root = &stage.root.children[0];
        assert_eq!(root.name, "Root");
        assert_eq!(root.children.len(), 1);
        assert_eq!(root.children[0].name, "Child");
        assert_eq!(root.children[0].path, "/Root/Child");
        assert_eq!(stage.meshes.len(), 1);
        assert_eq!(stage.meshes[0].path, "/Root/Child");
    }

    #[test]
    fn parse_empty_input_is_empty_stage() {
        let stage = parse("").unwrap();
        assert!(stage.meshes.is_empty());
        assert!(stage.materials.is_empty());
        assert!(stage.root.children.is_empty());
    }
}
