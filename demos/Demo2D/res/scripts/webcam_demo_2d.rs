use perro_api::prelude::*;

type SelfNodeType = Node2D;

const DEVICE_LABEL_NODE_NAME: &str = "WebcamDevices2DLabel";
const WEBCAM_NODE_NAME: &str = "DemoWebcam";

lifecycle!({
    fn on_init(&self, ctx: &mut ScriptContext<'_, API>) {
        let (text, first_slot) = webcam_device_report(ctx);
        if let (Some(slot), Some(webcam_id)) =
            (first_slot.as_deref(), get_child!(ctx.run, ctx.id, WEBCAM_NODE_NAME))
        {
            with_node_mut!(ctx.run, Webcam, webcam_id, |webcam| {
                webcam.config.device = slot.to_string().into();
            });
            log_info!("[Demo2D] webcam selected slot=\"{slot}\"");
        }
        log_info!("[Demo2D] {text}");
        if let Some(label_id) = get_child!(ctx.run, ctx.id, DEVICE_LABEL_NODE_NAME) {
            with_node_mut!(ctx.run, Label2D, label_id, |label| {
                label.text = text.into();
            });
        }
    }
});

fn webcam_device_report<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
) -> (String, Option<String>) {
    match ctx.res.Webcams().devices() {
        Ok(devices) if devices.is_empty() => {
            (
                "Webcams: 0\nslot \"\" auto-picks first device\nNo devices found".to_string(),
                None,
            )
        }
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
                    "[Demo2D] webcam slot=\"{}\" idx={} name=\"{}\" desc=\"{}\" extra=\"{}\"",
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
