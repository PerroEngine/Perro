use perro_input::InputAPI;
use perro_resource_context::api::ResourceAPI;
use perro_runtime_context::{RuntimeWindow, api::RuntimeAPI};
use perro_scripting::ScriptContext;

fn with_run<RT: RuntimeAPI + ?Sized>(run: &mut RuntimeWindow<'_, RT>, f: impl FnOnce()) {
    let _ = run;
    f();
}

fn fail_capture<RT, RS, IP>(ctx: &mut ScriptContext<'_, RT, RS, IP>)
where
    RT: RuntimeAPI + ?Sized,
    RS: ResourceAPI + ?Sized,
    IP: InputAPI + ?Sized,
{
    with_run(ctx.run, || {
        let _ = ctx.run.runtime_mut();
    });
}

fn main() {}
