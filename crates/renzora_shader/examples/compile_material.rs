use renzora_shader::material::codegen;
use renzora_shader::material::graph::MaterialGraph;

fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("usage: compile_material <path/to/file.material>");
    let text = std::fs::read_to_string(&path).expect("read material");
    let graph: MaterialGraph = serde_json::from_str(&text).expect("parse graph");

    let result = codegen::compile_with_functions(&graph, None);

    println!("domain: {:?}", result.domain);
    println!("requires_transmission: {}", result.requires_transmission);
    println!("texture_bindings ({}):", result.texture_bindings.len());
    for tb in &result.texture_bindings {
        println!(
            "  binding={} kind={:?} name={} path={}",
            tb.binding, tb.kind, tb.name, tb.asset_path
        );
    }
    println!("parameters ({}):", result.parameters.len());
    for p in &result.parameters {
        println!("  {} ({:?}) = {:?}", p.name, p.kind, p.default);
    }
    println!("errors ({}):", result.errors.len());
    for e in &result.errors {
        println!("  ERROR: {}", e);
    }
    println!("warnings ({}):", result.warnings.len());
    for w in &result.warnings {
        println!("  WARN: {}", w);
    }
    println!(
        "\n=== fragment_shader ({} bytes) ===",
        result.fragment_shader.len()
    );
    println!("{}", result.fragment_shader);
}
