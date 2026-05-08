use std::borrow::Cow;

#[derive(Clone, Debug, Default)]
pub struct AnimationBlendTreeDef {
    pub slots: Cow<'static, [Cow<'static, str>]>,
}

pub fn parse_panimtree(source: &str) -> Result<AnimationBlendTreeDef, String> {
    let mut slots = Vec::new();
    for line in source.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=')
            && k.trim() == "slot"
        {
            slots.push(Cow::Owned(v.trim().to_string()));
        }
    }
    if slots.is_empty() {
        return Err("panimtree contains no slots".to_string());
    }
    Ok(AnimationBlendTreeDef {
        slots: Cow::Owned(slots),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_slots() {
        let src = "slot = Idle\nslot = Walk\nslot = Run";
        let def = parse_panimtree(src).unwrap();
        assert_eq!(def.slots.len(), 3);
    }
}
