use perro_input::InputAPI;
use perro_resource_context::api::ResourceAPI;
use perro_runtime_context::{RuntimeWindow, api::RuntimeAPI};
use perro_scripting::{ScriptAPI, ScriptContext};

struct Api<RT: ?Sized, RS: ?Sized, IP: ?Sized>(std::marker::PhantomData<(*const RT, *const RS, *const IP)>);

impl<RT, RS, IP> ScriptAPI for Api<RT, RS, IP>
where
    RT: RuntimeAPI + ?Sized,
    RS: ResourceAPI + ?Sized,
    IP: InputAPI + ?Sized,
{
    type RT = RT;
    type RS = RS;
    type IP = IP;
}

fn with_run<RT: RuntimeAPI + ?Sized>(run: &mut RuntimeWindow<'_, RT>, f: impl FnOnce()) {
    let _ = run;
    f();
}

type Ctx<'a, RT, RS, IP> = ScriptContext<'a, Api<RT, RS, IP>>;

fn ok_outside<RT, RS, IP>(ctx: &mut Ctx<'_, RT, RS, IP>)
where
    RT: RuntimeAPI + ?Sized,
    RS: ResourceAPI + ?Sized,
    IP: InputAPI + ?Sized,
{
    let _ = (ctx.res, ctx.ipt);
    with_run(ctx.run, || {});
}

fn main() {}
