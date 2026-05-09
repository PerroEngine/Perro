use perro_api::prelude::*;

type SelfNodeType = UiPanel;

lifecycle!({
    fn on_update(
        &self,
        ctx: &mut ScriptContext<'_, API>,
    ) {
        if key_pressed!(ctx.ipt, KeyCode::Digit1) {
            locale_set!(ctx.res, Locale::EN);
        }
        if key_pressed!(ctx.ipt, KeyCode::Digit2) {
            locale_set!(ctx.res, Locale::ES);
        }
        if key_pressed!(ctx.ipt, KeyCode::Digit3) {
            locale_set!(ctx.res, Locale::FR);
        }
    }
});
