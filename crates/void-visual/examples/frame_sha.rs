fn main() {
    let mut scene = void_visual::Scene::new("p", 1);
    scene.apply(&void_visual::VisualCommand {
        command_id: "c1".into(),
        transaction_id: "t".into(),
        project_id: "p".into(),
        engine_epoch: 1,
        expected_revision: 0,
        op: void_visual::VisualOp::AddVisualLayerOp {
            layer_id: "g".into(),
            kind: void_visual::VisualLayerKind::Generator,
            name: "g".into(),
            index: 0,
            channel: void_visual::VisualChannel::Program,
            generator: Some(void_visual::GeneratorSpec {
                preset: "color-bars".into(),
                seed: 1,
                param_json: "{}".into(),
            }),
        },
    });
    let mut r = void_visual::Renderer::new_headless(320, 180).unwrap();
    println!(
        "adapter: {} | {} | {}",
        r.adapter_info().name,
        r.adapter_info().backend,
        r.adapter_info().driver_info
    );
    let plan = scene.resolve(void_visual::VisualChannel::Program, 0);
    let f = r.render(&plan, 0, 0, 0, 1, 0.0, true).unwrap();
    println!("frame_sha256: {}", f.frame_sha256);
}
