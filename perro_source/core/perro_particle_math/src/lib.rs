#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Op {
    Const(f32),
    T,
    Life,
    Lifetime,
    AgeLeft,
    Age01,
    SpawnTime,
    EmitterTime,
    Speed,
    Id,
    DirX,
    DirY,
    DirZ,
    VelX,
    VelY,
    VelZ,
    Rand,
    Rand2,
    Rand3,
    Seed,
    RingU,
    Index01,
    EmitterX,
    EmitterY,
    EmitterZ,
    Param,
    Add,
    Sub,
    Mul,
    Div,
    Pow,
    Neg,
    Sin,
    Cos,
    Tan,
    Abs,
    Sqrt,
    Min,
    Max,
    Clamp,
    Hash,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    ops: Vec<Op>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParticleEvalInput<'a> {
    pub t: f32,
    pub life: f32,
    pub lifetime: f32,
    pub spawn_time: f32,
    pub emitter_time: f32,
    pub speed: f32,
    pub particle_id: f32,
    pub dir: [f32; 3],
    pub vel: [f32; 3],
    pub rand: [f32; 3],
    pub seed: f32,
    pub ring_u: f32,
    pub index01: f32,
    pub emitter_pos: [f32; 3],
    pub prev_pos: [f32; 3],
    pub params: &'a [f32],
}

impl Program {
    pub fn new(ops: Vec<Op>) -> Self {
        Self { ops }
    }

    pub fn ops(&self) -> &[Op] {
        &self.ops
    }

    pub fn eval(&self, t: f32, life: f32, params: &[f32], stack: &mut Vec<f32>) -> Option<f32> {
        eval_ops(&self.ops, t, life, params, stack)
    }

    pub fn eval_particle(&self, input: &ParticleEvalInput<'_>, stack: &mut Vec<f32>) -> Option<f32> {
        eval_ops_particle(&self.ops, input, stack)
    }

    pub fn emit_wgsl_expr(&self) -> Result<String, CompileError> {
        emit_wgsl_expr_ops(&self.ops)
    }
}

pub fn eval_ops(
    ops: &[Op],
    t: f32,
    life: f32,
    params: &[f32],
    stack: &mut Vec<f32>,
) -> Option<f32> {
    let input = ParticleEvalInput {
        t,
        life,
        lifetime: life,
        spawn_time: 0.0,
        emitter_time: 0.0,
        speed: 0.0,
        particle_id: 0.0,
        dir: [0.0, 0.0, 0.0],
        vel: [0.0, 0.0, 0.0],
        rand: [0.0, 0.0, 0.0],
        seed: 0.0,
        ring_u: 0.0,
        index01: 0.0,
        emitter_pos: [0.0, 0.0, 0.0],
        prev_pos: [0.0, 0.0, 0.0],
        params,
    };
    eval_ops_particle(ops, &input, stack)
}

pub fn eval_ops_particle(
    ops: &[Op],
    input: &ParticleEvalInput<'_>,
    stack: &mut Vec<f32>,
) -> Option<f32> {
    let t = input.t;
    let life = input.life;
    let lifetime = input.lifetime;
    let spawn_time = input.spawn_time;
    let emitter_time = input.emitter_time;
    let speed = input.speed;
    let particle_id = input.particle_id;
    let dir = input.dir;
    let vel = input.vel;
    let rand = input.rand;
    let seed = input.seed;
    let ring_u = input.ring_u;
    let index01 = input.index01;
    let emitter_pos = input.emitter_pos;
    let params = input.params;
    stack.clear();
    for op in ops {
        match *op {
            Op::Const(v) => stack.push(v),
            Op::T => stack.push(t),
            Op::Life => stack.push(life),
            Op::Lifetime => stack.push(lifetime),
            Op::AgeLeft => stack.push((lifetime - life).max(0.0)),
            Op::Age01 => stack.push(t),
            Op::SpawnTime => stack.push(spawn_time),
            Op::EmitterTime => stack.push(emitter_time),
            Op::Speed => stack.push(speed),
            Op::Id => stack.push(particle_id),
            Op::DirX => stack.push(dir[0]),
            Op::DirY => stack.push(dir[1]),
            Op::DirZ => stack.push(dir[2]),
            Op::VelX => stack.push(vel[0]),
            Op::VelY => stack.push(vel[1]),
            Op::VelZ => stack.push(vel[2]),
            Op::Rand => stack.push(rand[0]),
            Op::Rand2 => stack.push(rand[1]),
            Op::Rand3 => stack.push(rand[2]),
            Op::Seed => stack.push(seed),
            Op::RingU => stack.push(ring_u),
            Op::Index01 => stack.push(index01),
            Op::EmitterX => stack.push(emitter_pos[0]),
            Op::EmitterY => stack.push(emitter_pos[1]),
            Op::EmitterZ => stack.push(emitter_pos[2]),
            Op::Param => {
                let idx = stack.pop()?.floor() as isize;
                if idx < 0 {
                    stack.push(0.0);
                } else {
                    stack.push(*params.get(idx as usize).unwrap_or(&0.0));
                }
            }
            Op::Add => {
                let b = stack.pop()?;
                let a = stack.pop()?;
                stack.push(a + b);
            }
            Op::Sub => {
                let b = stack.pop()?;
                let a = stack.pop()?;
                stack.push(a - b);
            }
            Op::Mul => {
                let b = stack.pop()?;
                let a = stack.pop()?;
                stack.push(a * b);
            }
            Op::Div => {
                let b = stack.pop()?;
                let a = stack.pop()?;
                stack.push(a / b);
            }
            Op::Pow => {
                let b = stack.pop()?;
                let a = stack.pop()?;
                stack.push(a.powf(b));
            }
            Op::Neg => {
                let a = stack.pop()?;
                stack.push(-a);
            }
            Op::Sin => {
                let a = stack.pop()?;
                stack.push(a.sin());
            }
            Op::Cos => {
                let a = stack.pop()?;
                stack.push(a.cos());
            }
            Op::Tan => {
                let a = stack.pop()?;
                stack.push(a.tan());
            }
            Op::Abs => {
                let a = stack.pop()?;
                stack.push(a.abs());
            }
            Op::Sqrt => {
                let a = stack.pop()?;
                stack.push(a.max(0.0).sqrt());
            }
            Op::Min => {
                let b = stack.pop()?;
                let a = stack.pop()?;
                stack.push(a.min(b));
            }
            Op::Max => {
                let b = stack.pop()?;
                let a = stack.pop()?;
                stack.push(a.max(b));
            }
            Op::Clamp => {
                let hi = stack.pop()?;
                let lo = stack.pop()?;
                let x = stack.pop()?;
                stack.push(x.clamp(lo, hi));
            }
            Op::Hash => {
                let a = stack.pop()?;
                stack.push(hash01f(a));
            }
        }
    }
    if stack.len() == 1 { stack.pop() } else { None }
}

pub fn emit_wgsl_expr_ops(ops: &[Op]) -> Result<String, CompileError> {
    let mut stack: Vec<String> = Vec::new();
    for op in ops {
        match *op {
            Op::Const(v) => stack.push(format_float(v)),
            Op::T => stack.push("t".to_string()),
            Op::Life => stack.push("life".to_string()),
            Op::Lifetime => stack.push("lifetime".to_string()),
            Op::AgeLeft => stack.push("age_left".to_string()),
            Op::Age01 => stack.push("age01".to_string()),
            Op::SpawnTime => stack.push("spawn_time".to_string()),
            Op::EmitterTime => stack.push("emitter_time".to_string()),
            Op::Speed => stack.push("speed".to_string()),
            Op::Id => stack.push("particle_id".to_string()),
            Op::DirX => stack.push("dir_x".to_string()),
            Op::DirY => stack.push("dir_y".to_string()),
            Op::DirZ => stack.push("dir_z".to_string()),
            Op::VelX => stack.push("vel_x".to_string()),
            Op::VelY => stack.push("vel_y".to_string()),
            Op::VelZ => stack.push("vel_z".to_string()),
            Op::Rand => stack.push("rand0".to_string()),
            Op::Rand2 => stack.push("rand1".to_string()),
            Op::Rand3 => stack.push("rand2".to_string()),
            Op::Seed => stack.push("seed".to_string()),
            Op::RingU => stack.push("ring_u".to_string()),
            Op::Index01 => stack.push("index01".to_string()),
            Op::EmitterX => stack.push("emitter_x".to_string()),
            Op::EmitterY => stack.push("emitter_y".to_string()),
            Op::EmitterZ => stack.push("emitter_z".to_string()),
            Op::Param => {
                let idx = stack.pop().ok_or(CompileError::InvalidProgram)?;
                stack.push(format!("params_expr({idx}, params_len, params)"));
            }
            Op::Add => push_bin(&mut stack, "+")?,
            Op::Sub => push_bin(&mut stack, "-")?,
            Op::Mul => push_bin(&mut stack, "*")?,
            Op::Div => push_bin(&mut stack, "/")?,
            Op::Pow => {
                let b = stack.pop().ok_or(CompileError::InvalidProgram)?;
                let a = stack.pop().ok_or(CompileError::InvalidProgram)?;
                stack.push(format!("pow({a}, {b})"));
            }
            Op::Neg => {
                let a = stack.pop().ok_or(CompileError::InvalidProgram)?;
                stack.push(format!("(-{a})"));
            }
            Op::Sin => push_unary(&mut stack, "sin")?,
            Op::Cos => push_unary(&mut stack, "cos")?,
            Op::Tan => push_unary(&mut stack, "tan")?,
            Op::Abs => push_unary(&mut stack, "abs")?,
            Op::Sqrt => {
                let a = stack.pop().ok_or(CompileError::InvalidProgram)?;
                stack.push(format!("sqrt(max({a}, 0.0))"));
            }
            Op::Min => {
                let b = stack.pop().ok_or(CompileError::InvalidProgram)?;
                let a = stack.pop().ok_or(CompileError::InvalidProgram)?;
                stack.push(format!("min({a}, {b})"));
            }
            Op::Max => {
                let b = stack.pop().ok_or(CompileError::InvalidProgram)?;
                let a = stack.pop().ok_or(CompileError::InvalidProgram)?;
                stack.push(format!("max({a}, {b})"));
            }
            Op::Clamp => {
                let hi = stack.pop().ok_or(CompileError::InvalidProgram)?;
                let lo = stack.pop().ok_or(CompileError::InvalidProgram)?;
                let x = stack.pop().ok_or(CompileError::InvalidProgram)?;
                stack.push(format!("clamp({x}, {lo}, {hi})"));
            }
            Op::Hash => {
                let a = stack.pop().ok_or(CompileError::InvalidProgram)?;
                stack.push(format!("hash01f({a})"));
            }
        }
    }
    if stack.len() == 1 {
        Ok(stack.pop().unwrap_or_default())
    } else {
        Err(CompileError::InvalidProgram)
    }
}

fn push_bin(stack: &mut Vec<String>, op: &str) -> Result<(), CompileError> {
    let b = stack.pop().ok_or(CompileError::InvalidProgram)?;
    let a = stack.pop().ok_or(CompileError::InvalidProgram)?;
    stack.push(format!("({a} {op} {b})"));
    Ok(())
}

fn push_unary(stack: &mut Vec<String>, func: &str) -> Result<(), CompileError> {
    let a = stack.pop().ok_or(CompileError::InvalidProgram)?;
    stack.push(format!("{func}({a})"));
    Ok(())
}

fn format_float(v: f32) -> String {
    if v.is_finite() {
        if v.fract() == 0.0 {
            format!("{v:.1}")
        } else {
            v.to_string()
        }
    } else {
        "0.0".to_string()
    }
}

#[inline]
fn hash01f(v: f32) -> f32 {
    let n = (v * 12.9898 + 78.233).sin() * 43_758.547;
    n - n.floor()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompileError {
    UnexpectedToken,
    UnexpectedEnd,
    UnknownIdentifier,
    InvalidFunctionArity,
    InvalidProgram,
}

pub fn compile_expression(expr: &str) -> Result<Program, CompileError> {
    let mut c = Compiler {
        s: expr.as_bytes(),
        i: 0,
        ops: Vec::new(),
    };
    c.parse_expr()?;
    c.skip_ws();
    if c.i == c.s.len() {
        Ok(Program::new(c.ops))
    } else {
        Err(CompileError::UnexpectedToken)
    }
}

struct Compiler<'a> {
    s: &'a [u8],
    i: usize,
    ops: Vec<Op>,
}

impl<'a> Compiler<'a> {
    fn skip_ws(&mut self) {
        while self.i < self.s.len() && self.s[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }

    fn parse_expr(&mut self) -> Result<(), CompileError> {
        self.parse_term()?;
        loop {
            self.skip_ws();
            if self.eat(b'+') {
                self.parse_term()?;
                self.ops.push(Op::Add);
            } else if self.eat(b'-') {
                self.parse_term()?;
                self.ops.push(Op::Sub);
            } else {
                break;
            }
        }
        Ok(())
    }

    fn parse_term(&mut self) -> Result<(), CompileError> {
        self.parse_power()?;
        loop {
            self.skip_ws();
            if self.eat(b'*') {
                self.parse_power()?;
                self.ops.push(Op::Mul);
            } else if self.eat(b'/') {
                self.parse_power()?;
                self.ops.push(Op::Div);
            } else {
                break;
            }
        }
        Ok(())
    }

    fn parse_power(&mut self) -> Result<(), CompileError> {
        self.parse_unary()?;
        self.skip_ws();
        while self.eat(b'^') {
            self.parse_unary()?;
            self.ops.push(Op::Pow);
            self.skip_ws();
        }
        Ok(())
    }

    fn parse_unary(&mut self) -> Result<(), CompileError> {
        self.skip_ws();
        if self.eat(b'+') {
            return self.parse_unary();
        }
        if self.eat(b'-') {
            self.parse_unary()?;
            self.ops.push(Op::Neg);
            return Ok(());
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<(), CompileError> {
        self.skip_ws();
        if self.eat(b'(') {
            self.parse_expr()?;
            self.skip_ws();
            self.expect(b')')?;
            return Ok(());
        }
        if let Some(n) = self.parse_number() {
            self.ops.push(Op::Const(n));
            return Ok(());
        }

        let ident = self.parse_ident().ok_or(CompileError::UnexpectedToken)?;
        self.skip_ws();

        if ident == "params" && self.eat(b'[') {
            self.parse_expr()?;
            self.skip_ws();
            self.expect(b']')?;
            self.ops.push(Op::Param);
            return Ok(());
        }

        if self.eat(b'(') {
            let argc = self.parse_args()?;
            let op = match (ident.as_str(), argc) {
                ("sin", 1) => Op::Sin,
                ("cos", 1) => Op::Cos,
                ("tan", 1) => Op::Tan,
                ("abs", 1) => Op::Abs,
                ("sqrt", 1) => Op::Sqrt,
                ("min", 2) => Op::Min,
                ("max", 2) => Op::Max,
                ("clamp", 3) => Op::Clamp,
                ("hash", 1) => Op::Hash,
                _ => return Err(CompileError::InvalidFunctionArity),
            };
            self.ops.push(op);
            return Ok(());
        }

        match ident.as_str() {
            "t" => self.ops.push(Op::T),
            "life" => self.ops.push(Op::Life),
            "lifetime" => self.ops.push(Op::Lifetime),
            "age_left" => self.ops.push(Op::AgeLeft),
            "age01" => self.ops.push(Op::Age01),
            "spawn_time" => self.ops.push(Op::SpawnTime),
            "emitter_time" => self.ops.push(Op::EmitterTime),
            "speed" => self.ops.push(Op::Speed),
            "id" => self.ops.push(Op::Id),
            "dir_x" => self.ops.push(Op::DirX),
            "dir_y" => self.ops.push(Op::DirY),
            "dir_z" => self.ops.push(Op::DirZ),
            "vel_x" => self.ops.push(Op::VelX),
            "vel_y" => self.ops.push(Op::VelY),
            "vel_z" => self.ops.push(Op::VelZ),
            "rand" => self.ops.push(Op::Rand),
            "rand2" => self.ops.push(Op::Rand2),
            "rand3" => self.ops.push(Op::Rand3),
            "seed" => self.ops.push(Op::Seed),
            "ring_u" => self.ops.push(Op::RingU),
            "index01" => self.ops.push(Op::Index01),
            "emitter_x" => self.ops.push(Op::EmitterX),
            "emitter_y" => self.ops.push(Op::EmitterY),
            "emitter_z" => self.ops.push(Op::EmitterZ),
            "pi" => self.ops.push(Op::Const(std::f32::consts::PI)),
            "tau" => self.ops.push(Op::Const(std::f32::consts::TAU)),
            _ => return Err(CompileError::UnknownIdentifier),
        }
        Ok(())
    }

    fn parse_args(&mut self) -> Result<usize, CompileError> {
        let mut argc = 0usize;
        self.skip_ws();
        if self.eat(b')') {
            return Ok(argc);
        }
        loop {
            self.parse_expr()?;
            argc += 1;
            self.skip_ws();
            if self.eat(b',') {
                continue;
            }
            self.expect(b')')?;
            break;
        }
        Ok(argc)
    }

    fn parse_ident(&mut self) -> Option<String> {
        self.skip_ws();
        let start = self.i;
        while self.i < self.s.len()
            && (self.s[self.i].is_ascii_alphanumeric() || self.s[self.i] == b'_')
        {
            self.i += 1;
        }
        (self.i > start).then(|| String::from_utf8_lossy(&self.s[start..self.i]).to_string())
    }

    fn parse_number(&mut self) -> Option<f32> {
        self.skip_ws();
        let start = self.i;
        let mut seen = false;
        while self.i < self.s.len() {
            let c = self.s[self.i];
            if c.is_ascii_digit() || c == b'.' {
                seen = true;
                self.i += 1;
            } else {
                break;
            }
        }
        if !seen {
            self.i = start;
            return None;
        }
        let s = std::str::from_utf8(&self.s[start..self.i]).ok()?;
        s.parse::<f32>().ok()
    }

    fn eat(&mut self, c: u8) -> bool {
        self.skip_ws();
        if self.i < self.s.len() && self.s[self.i] == c {
            self.i += 1;
            true
        } else {
            false
        }
    }

    fn expect(&mut self, c: u8) -> Result<(), CompileError> {
        if self.eat(c) {
            Ok(())
        } else {
            Err(CompileError::UnexpectedEnd)
        }
    }
}

#[cfg(test)]
#[path = "../tests/unit/lib_tests.rs"]
mod tests;
