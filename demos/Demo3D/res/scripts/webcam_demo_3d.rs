use perro_api::prelude::*;

type SelfNodeType = Node3D;

#[State]
struct WebcamDemo3DState {
    #[default = NodeID::nil()]
    webcam: NodeID,
    #[default = NodeID::nil()]
    device_label: NodeID,
}

lifecycle!({
    fn on_init(&self, ctx: &mut ScriptContext<'_, API>) {
        let (text, first_slot) = webcam_device_report(ctx);
        let (webcam_id, label_id) = with_state!(ctx.run, WebcamDemo3DState, ctx.id, |state| {
            (state.webcam, state.device_label)
        }).unwrap_or_default();
        if let Some(slot) = first_slot.as_deref().filter(|_| !webcam_id.is_nil()) {
            with_node_mut!(ctx.run, Webcam, webcam_id, |webcam| {
                webcam.config.device = slot.to_string().into();
            });
            log_info!("[Demo3D] webcam selected slot=\"{slot}\"");
        }
        log_info!("[Demo3D] {text}");
        if !label_id.is_nil() {
            with_node_mut!(ctx.run, Label3D, label_id, |label| {
                label.text = text.into();
            });
        }
    }
});

fn webcam_device_report<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
) -> (String, Option<String>) {
    match ctx.res.Webcams().devices() {
        Ok(devices) if devices.is_empty() => (
            "Webcams: 0\nslot \"\" auto-picks first device\nNo devices found".to_string(),
            None,
        ),
        Ok(devices) => {
            let first_slot = devices.first().map(|device| device.slot.clone());
            let mut out = format!(
                "Webcams: {}\nslot \"\" auto-picks first device",
                devices.len()
            );
            for device in devices.iter().take(4) {
                let idx = device
                    .index
                    .map(|idx| idx.to_string())
                    .unwrap_or_else(|| "-".to_string());
                out.push_str(&format!(
                    "\nslot \"{}\" idx {} {}",
                    device.slot, idx, device.name
                ));
                log_info!(
                    "[Demo3D] webcam slot=\"{}\" idx={} name=\"{}\" desc=\"{}\" extra=\"{}\"",
                    device.slot,
                    idx,
                    device.name,
                    device.description,
                    device.extra
                );
            }
            (out, first_slot)
        }
        Err(err) => (format!("Webcam query failed\n{err}"), None),
    }
}
