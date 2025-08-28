use serde::{Deserialize, Serialize};
use wgpu::RenderPass;

use crate::{ui_element::{BaseElement, UIElement}, ui_elements::ui_panel::UIPanel, ui_node::Ui, Color, Graphics};




pub fn render_ui(ui_node: &Ui, gfx: &mut Graphics, pass: &mut RenderPass<'_>) {
        // Iterate over all UI elements in the node and draw them
        for (id, element) in &ui_node.elements {
            // Early out if element is invisible
            if !element.get_visible() {
                continue;
            }

            match element {
                UIElement::Panel(panel) => {
                    render_panel(panel, gfx, pass);
                }
                // Add more UIElement variants here (e.g. Button, Label, etc.)
            }
        }
    }

fn render_panel(panel: &UIPanel, gfx: &mut Graphics, pass: &mut RenderPass<'_>) {
    // Extract props info
    let background_color = panel
        .props
        .background_color
        .clone()
        .unwrap_or(Color::new(0, 0, 0, 0));
    let corner_radius = panel.props.corner_radius;
    let border_color = panel.props.border_color.clone();
    let border_thickness = panel.props.border_thickness;

    // Step 1: Draw background rectangle
    gfx.draw_rect(
        pass,
        panel.global_transform.clone(),
        panel.size.clone(),
        panel.pivot,
        background_color,
        Some(corner_radius),
    );

    // Step 2: Optional border (only if thickness > 0 and color is set)
    if border_thickness > 0.0 {
        if let Some(border_color) = border_color {
            gfx.draw_border(
                pass,
                panel.global_transform.clone(),
                panel.size.clone(),
                panel.pivot,
                border_color,
                border_thickness,
                Some(corner_radius),
            );
        }
    }
}