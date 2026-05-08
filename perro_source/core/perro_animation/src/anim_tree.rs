use perro_scene::{Lexer, Token};
use std::borrow::Cow;
use std::collections::HashSet;

#[derive(Clone, Debug, Default)]
pub struct AnimationTreeAsset {
    pub name: Cow<'static, str>,
    pub slots: Cow<'static, [AnimationTreeSlot]>,
    pub nodes: Cow<'static, [AnimationTreeGraphNode]>,
    pub output: Cow<'static, str>,
}

#[derive(Clone, Debug, Default)]
pub struct AnimationTreeSlot {
    pub name: Cow<'static, str>,
}

#[derive(Clone, Debug)]
pub struct AnimationTreeGraphNode {
    pub key: Cow<'static, str>,
    pub kind: AnimationTreeNodeKind,
}

impl Default for AnimationTreeGraphNode {
    fn default() -> Self {
        Self {
            key: Cow::Borrowed(""),
            kind: AnimationTreeNodeKind::Slot {
                slot: Cow::Borrowed(""),
            },
        }
    }
}

#[derive(Clone, Debug)]
pub enum AnimationTreeNodeKind {
    Slot {
        slot: Cow<'static, str>,
    },
    Blend {
        inputs: Cow<'static, [Cow<'static, str>]>,
        weights: Cow<'static, [f32]>,
        mask: AnimationTreeMask,
    },
    Add {
        base: Cow<'static, str>,
        inputs: Cow<'static, [Cow<'static, str>]>,
        weights: Cow<'static, [f32]>,
        mask: AnimationTreeMask,
    },
    Invert {
        input: Cow<'static, str>,
        mask: AnimationTreeMask,
    },
}

#[derive(Clone, Debug, Default)]
pub struct AnimationTreeMask {
    pub objects: Cow<'static, [Cow<'static, str>]>,
    pub fields: Cow<'static, [Cow<'static, str>]>,
    pub bones: Cow<'static, [Cow<'static, str>]>,
}

impl AnimationTreeMask {
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.objects.is_empty() && self.fields.is_empty() && self.bones.is_empty()
    }
}

pub fn parse_panimtree(src: &str) -> Result<AnimationTreeAsset, String> {
    AnimationTreeParser::new(src).parse()
}

struct AnimationTreeParser<'a> {
    lexer: Lexer<'a>,
    current: Token,
}

impl<'a> AnimationTreeParser<'a> {
    fn new(src: &'a str) -> Self {
        let mut lexer = Lexer::new(src);
        let current = lexer.next_token();
        Self { lexer, current }
    }

    fn parse(mut self) -> Result<AnimationTreeAsset, String> {
        let mut name = Cow::Borrowed("AnimationTree");
        let mut slots = Vec::new();
        let mut nodes = Vec::new();
        let mut output = Cow::Borrowed("");
        let mut seen_nodes = HashSet::<String>::new();

        while self.current != Token::Eof {
            if self.current != Token::LBracket {
                self.advance();
                continue;
            }
            self.advance();
            if self.current == Token::Slash {
                return Err("unexpected close block".to_string());
            }
            let block = self.expect_ident()?;
            self.expect(Token::RBracket)?;
            match block.as_str() {
                "AnimationTree" => {
                    name = self.parse_header_block()?;
                }
                "Slots" => {
                    slots = self.parse_slots_block()?;
                }
                "Output" => {
                    output = self.parse_output_block()?;
                }
                key => {
                    if !seen_nodes.insert(key.to_string()) {
                        return Err(format!("duplicate animation tree node `{key}`"));
                    }
                    let node = self.parse_graph_node_block(key.to_string())?;
                    nodes.push(node);
                }
            }
        }

        if output.is_empty() {
            return Err("animation tree missing [Output]".to_string());
        }

        let keys = nodes
            .iter()
            .map(|n| n.key.as_ref().to_string())
            .collect::<HashSet<_>>();
        if !keys.contains(output.as_ref()) {
            return Err(format!("unknown animation tree output `@{}`", output));
        }
        for node in &nodes {
            validate_node_refs(node, &keys)?;
        }

        Ok(AnimationTreeAsset {
            name,
            slots: Cow::Owned(slots),
            nodes: Cow::Owned(nodes),
            output,
        })
    }

    fn parse_header_block(&mut self) -> Result<Cow<'static, str>, String> {
        let mut name = Cow::Borrowed("AnimationTree");
        loop {
            if self.consume_close("AnimationTree")? {
                break;
            }
            let key = self.expect_ident()?;
            self.expect(Token::Equals)?;
            if key == "name" {
                name = Cow::Owned(self.expect_text_like()?);
            } else {
                self.skip_value()?;
            }
        }
        Ok(name)
    }

    fn parse_slots_block(&mut self) -> Result<Vec<AnimationTreeSlot>, String> {
        let mut slots = Vec::new();
        loop {
            if self.consume_close("Slots")? {
                break;
            }
            match &self.current {
                Token::Ident(v) | Token::String(v) => {
                    slots.push(AnimationTreeSlot {
                        name: Cow::Owned(v.clone()),
                    });
                    self.advance();
                }
                Token::Comma => self.advance(),
                other => return Err(format!("expected slot name, got {other:?}")),
            }
        }
        Ok(slots)
    }

    fn parse_output_block(&mut self) -> Result<Cow<'static, str>, String> {
        let mut output = Cow::Borrowed("");
        loop {
            if self.consume_close("Output")? {
                break;
            }
            let key = self.expect_ident()?;
            self.expect(Token::Equals)?;
            if key == "input" {
                output = Cow::Owned(self.expect_ref()?);
            } else {
                self.skip_value()?;
            }
        }
        Ok(output)
    }

    fn parse_graph_node_block(&mut self, key: String) -> Result<AnimationTreeGraphNode, String> {
        self.expect(Token::LBracket)?;
        let kind = self.expect_ident()?;
        self.expect(Token::RBracket)?;
        let node_kind = match kind.as_str() {
            "Slot" => self.parse_slot_kind()?,
            "Blend" | "BlendN" => self.parse_blend_kind(&kind)?,
            "Add" | "AddN" => self.parse_add_kind(&kind)?,
            "Invert" => self.parse_invert_kind()?,
            other => return Err(format!("unsupported animation tree node kind `{other}`")),
        };
        self.expect(Token::LBracket)?;
        self.expect(Token::Slash)?;
        let end = self.expect_ident()?;
        self.expect(Token::RBracket)?;
        if end != key {
            return Err(format!("expected [/{key}], got [/{end}]"));
        }
        Ok(AnimationTreeGraphNode {
            key: Cow::Owned(key),
            kind: node_kind,
        })
    }

    fn parse_slot_kind(&mut self) -> Result<AnimationTreeNodeKind, String> {
        let mut slot = Cow::Borrowed("");
        loop {
            if self.consume_close("Slot")? {
                break;
            }
            let key = self.expect_ident()?;
            self.expect(Token::Equals)?;
            if key == "slot" {
                slot = Cow::Owned(self.expect_text_like()?);
            } else {
                self.skip_value()?;
            }
        }
        Ok(AnimationTreeNodeKind::Slot { slot })
    }

    fn parse_blend_kind(&mut self, close_block: &str) -> Result<AnimationTreeNodeKind, String> {
        let mut inputs = Vec::new();
        let mut weights = Vec::new();
        let mut mask = AnimationTreeMask::default();
        loop {
            if self.consume_close(close_block)? {
                break;
            }
            let key = self.expect_ident()?;
            self.expect(Token::Equals)?;
            match key.as_str() {
                "inputs" => inputs = self.parse_ref_list()?,
                "weights" => weights = self.parse_f32_list()?,
                "mask" => mask = self.parse_mask()?,
                _ => self.skip_value()?,
            }
        }
        Ok(AnimationTreeNodeKind::Blend {
            inputs: Cow::Owned(inputs.into_iter().map(Cow::Owned).collect()),
            weights: Cow::Owned(weights),
            mask,
        })
    }

    fn parse_add_kind(&mut self, close_block: &str) -> Result<AnimationTreeNodeKind, String> {
        let mut base = Cow::Borrowed("");
        let mut inputs = Vec::new();
        let mut weights = Vec::new();
        let mut mask = AnimationTreeMask::default();
        loop {
            if self.consume_close(close_block)? {
                break;
            }
            let key = self.expect_ident()?;
            self.expect(Token::Equals)?;
            match key.as_str() {
                "base" => base = Cow::Owned(self.expect_ref()?),
                "inputs" => inputs = self.parse_ref_list()?,
                "weights" => weights = self.parse_f32_list()?,
                "mask" => mask = self.parse_mask()?,
                _ => self.skip_value()?,
            }
        }
        Ok(AnimationTreeNodeKind::Add {
            base,
            inputs: Cow::Owned(inputs.into_iter().map(Cow::Owned).collect()),
            weights: Cow::Owned(weights),
            mask,
        })
    }

    fn parse_invert_kind(&mut self) -> Result<AnimationTreeNodeKind, String> {
        let mut input = Cow::Borrowed("");
        let mut mask = AnimationTreeMask::default();
        loop {
            if self.consume_close("Invert")? {
                break;
            }
            let key = self.expect_ident()?;
            self.expect(Token::Equals)?;
            match key.as_str() {
                "input" => input = Cow::Owned(self.expect_ref()?),
                "mask" => mask = self.parse_mask()?,
                _ => self.skip_value()?,
            }
        }
        Ok(AnimationTreeNodeKind::Invert { input, mask })
    }

    fn parse_ref_list(&mut self) -> Result<Vec<String>, String> {
        self.expect(Token::LBracket)?;
        let mut refs = Vec::new();
        loop {
            if self.current == Token::RBracket {
                self.advance();
                break;
            }
            refs.push(self.expect_ref()?);
            if self.current == Token::Comma {
                self.advance();
            }
        }
        Ok(refs)
    }

    fn parse_f32_list(&mut self) -> Result<Vec<f32>, String> {
        self.expect(Token::LBracket)?;
        let mut out = Vec::new();
        loop {
            if self.current == Token::RBracket {
                self.advance();
                break;
            }
            let Token::Number(v) = self.current else {
                return Err(format!("expected number, got {:?}", self.current));
            };
            out.push(v);
            self.advance();
            if self.current == Token::Comma {
                self.advance();
            }
        }
        Ok(out)
    }

    fn parse_mask(&mut self) -> Result<AnimationTreeMask, String> {
        self.expect(Token::LBrace)?;
        let mut mask = AnimationTreeMask::default();
        loop {
            if self.current == Token::RBrace {
                self.advance();
                break;
            }
            let key = self.expect_ident()?;
            if matches!(self.current, Token::Equals | Token::Colon) {
                self.advance();
            } else {
                return Err(format!(
                    "expected `=` or `:` in mask, got {:?}",
                    self.current
                ));
            }
            let values = self.parse_name_list()?;
            match key.as_str() {
                "objects" => {
                    mask.objects = Cow::Owned(values.into_iter().map(Cow::Owned).collect())
                }
                "fields" => mask.fields = Cow::Owned(values.into_iter().map(Cow::Owned).collect()),
                "bones" => mask.bones = Cow::Owned(values.into_iter().map(Cow::Owned).collect()),
                _ => {}
            }
            if self.current == Token::Comma {
                self.advance();
            }
        }
        Ok(mask)
    }

    fn parse_name_list(&mut self) -> Result<Vec<String>, String> {
        self.expect(Token::LBracket)?;
        let mut out = Vec::new();
        loop {
            if self.current == Token::RBracket {
                self.advance();
                break;
            }
            out.push(self.expect_text_like()?);
            if self.current == Token::Comma {
                self.advance();
            }
        }
        Ok(out)
    }

    fn expect_ref(&mut self) -> Result<String, String> {
        self.expect(Token::At)?;
        self.expect_ident()
    }

    fn expect_text_like(&mut self) -> Result<String, String> {
        match std::mem::replace(&mut self.current, Token::Eof) {
            Token::Ident(v) | Token::String(v) => {
                self.advance();
                Ok(v)
            }
            other => Err(format!("expected name, got {other:?}")),
        }
    }

    fn skip_value(&mut self) -> Result<(), String> {
        match self.current {
            Token::LBracket => {
                let mut depth = 0i32;
                loop {
                    match self.current {
                        Token::LBracket => depth += 1,
                        Token::RBracket => {
                            depth -= 1;
                            self.advance();
                            if depth == 0 {
                                break;
                            }
                            continue;
                        }
                        Token::Eof => break,
                        _ => {}
                    }
                    self.advance();
                }
            }
            Token::LBrace => {
                let mut depth = 0i32;
                loop {
                    match self.current {
                        Token::LBrace => depth += 1,
                        Token::RBrace => {
                            depth -= 1;
                            self.advance();
                            if depth == 0 {
                                break;
                            }
                            continue;
                        }
                        Token::Eof => break,
                        _ => {}
                    }
                    self.advance();
                }
            }
            _ => self.advance(),
        }
        Ok(())
    }

    fn consume_close(&mut self, expected: &str) -> Result<bool, String> {
        if self.current != Token::LBracket {
            return Ok(false);
        }
        self.advance();
        if self.current != Token::Slash {
            return Err(format!("unexpected nested block in [{expected}]"));
        }
        self.advance();
        let end = self.expect_ident()?;
        self.expect(Token::RBracket)?;
        if end != expected {
            return Err(format!("expected [/{expected}], got [/{end}]"));
        }
        Ok(true)
    }

    fn expect(&mut self, token: Token) -> Result<(), String> {
        if self.current != token {
            return Err(format!("expected {token:?}, got {:?}", self.current));
        }
        self.advance();
        Ok(())
    }

    fn expect_ident(&mut self) -> Result<String, String> {
        match std::mem::replace(&mut self.current, Token::Eof) {
            Token::Ident(v) => {
                self.advance();
                Ok(v)
            }
            other => Err(format!("expected identifier, got {other:?}")),
        }
    }

    fn advance(&mut self) {
        self.current = self.lexer.next_token();
    }
}

fn validate_node_refs(node: &AnimationTreeGraphNode, keys: &HashSet<String>) -> Result<(), String> {
    let check = |value: &str| {
        if keys.contains(value) {
            Ok(())
        } else {
            Err(format!("unknown animation tree ref `@{value}`"))
        }
    };
    match &node.kind {
        AnimationTreeNodeKind::Slot { .. } => Ok(()),
        AnimationTreeNodeKind::Blend { inputs, .. } => {
            for input in inputs.iter() {
                check(input.as_ref())?;
            }
            Ok(())
        }
        AnimationTreeNodeKind::Add { base, inputs, .. } => {
            check(base.as_ref())?;
            for input in inputs.iter() {
                check(input.as_ref())?;
            }
            Ok(())
        }
        AnimationTreeNodeKind::Invert { input, .. } => check(input.as_ref()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_nested_blocks() {
        let src = r#"
[AnimationTree]
name = "Player"
[/AnimationTree]
[Slots]
Idle
Run
[/Slots]
[IdleSrc]
[Slot]
slot = Idle
[/Slot]
[/IdleSrc]
[RunSrc]
[Slot]
slot = Run
[/Slot]
[/RunSrc]
[MoveBlend]
[Blend]
inputs = [@IdleSrc, @RunSrc]
weights = [1.0, 0.0]
mask = { objects=[Hero], fields=[position, rotation] }
[/Blend]
[/MoveBlend]
[Output]
input = @MoveBlend
[/Output]
"#;
        let tree = parse_panimtree(src).expect("tree parse");
        assert_eq!(tree.name.as_ref(), "Player");
        assert_eq!(tree.slots.len(), 2);
        assert_eq!(tree.nodes.len(), 3);
        assert_eq!(tree.output.as_ref(), "MoveBlend");
    }

    #[test]
    fn reject_unknown_ref() {
        let src = r#"
[Slots]
Idle
[/Slots]
[IdleSrc]
[Slot]
slot = Idle
[/Slot]
[/IdleSrc]
[Output]
input = @Missing
[/Output]
"#;
        assert!(parse_panimtree(src).is_err());
    }
}
