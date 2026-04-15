// parser.rs - Parse into scene types
use crate::{
    Lexer, Scene, SceneKey, SceneNodeData, SceneNodeDataBase, SceneNodeEntry, SceneObjectField,
    SceneValue, SceneValueKey, Token,
};
use perro_structs::Quaternion;
use std::borrow::Cow;
use std::collections::HashMap;

pub struct Parser<'a> {
    src: &'a str,
    lexer: Lexer<'a>,
    current: Token,
    vars: HashMap<String, SceneValue>,
}

impl<'a> Parser<'a> {
    pub fn new(src: &'a str) -> Self {
        let mut lexer = Lexer::new(src);
        let current = lexer.next_token();
        Self {
            src,
            lexer,
            current,
            vars: HashMap::new(),
        }
    }

    fn advance(&mut self) {
        self.current = self.lexer.next_token();
    }

    fn expect(&mut self, t: Token) {
        if self.current != t {
            panic!("Expected {:?}, got {:?}", t, self.current);
        }
        self.advance();
    }

    fn expect_ident(&mut self) -> String {
        match std::mem::replace(&mut self.current, Token::Eof) {
            Token::Ident(s) => {
                self.advance();
                s
            }
            other => panic!("Expected identifier, got {:?}", other),
        }
    }

    fn collect_vars(mut self) -> HashMap<String, SceneValue> {
        while self.current != Token::Eof {
            if self.current == Token::At {
                self.advance();
                let name = self.expect_ident();
                if self.current == Token::Equals {
                    self.advance();
                    let value = self.parse_value();
                    self.vars.insert(name, value);
                }
                continue;
            }
            self.advance();
        }
        self.vars
    }

    fn parse_value(&mut self) -> SceneValue {
        match &self.current {
            Token::Number(n) => {
                let v = *n;
                self.advance();
                SceneValue::F32(v)
            }

            Token::String(s) => {
                let v = s.clone();
                self.advance();
                SceneValue::Str(Cow::Owned(v))
            }

            Token::At => {
                self.advance();
                let name = self.expect_ident();
                self.vars
                    .get(&name)
                    .cloned()
                    .unwrap_or_else(|| panic!("Unknown variable @{name}"))
            }

            Token::Ident(name) => {
                let key = name.clone();
                self.advance();
                SceneValue::Key(SceneValueKey::from(key))
            }

            Token::LParen => {
                self.advance();
                let mut nums = Vec::new();
                loop {
                    if let Token::Number(n) = self.current {
                        nums.push(n);
                        self.advance();
                    }
                    if self.current == Token::Comma {
                        self.advance();
                        continue;
                    }
                    break;
                }
                self.expect(Token::RParen);

                match nums.len() {
                    2 => SceneValue::Vec2 {
                        x: nums[0],
                        y: nums[1],
                    },
                    3 => SceneValue::Vec3 {
                        x: nums[0],
                        y: nums[1],
                        z: nums[2],
                    },
                    4 => SceneValue::Vec4 {
                        x: nums[0],
                        y: nums[1],
                        z: nums[2],
                        w: nums[3],
                    },
                    _ => panic!("Invalid vector length"),
                }
            }

            Token::True => {
                self.advance();
                SceneValue::Bool(true)
            }

            Token::False => {
                self.advance();
                SceneValue::Bool(false)
            }

            // inside parse_value(), in the Token::LBrace => { ... } arm
            Token::LBrace => {
                self.advance();
                let mut entries = Vec::new();
                loop {
                    if self.current == Token::RBrace {
                        self.advance();
                        break;
                    }

                    let key = match &self.current {
                        Token::Ident(name) => {
                            let out = name.clone();
                            self.advance();
                            out
                        }
                        Token::String(name) => {
                            let out = name.clone();
                            self.advance();
                            out
                        }
                        Token::Number(n) => {
                            let out = n.to_string();
                            self.advance();
                            out
                        }
                        other => panic!("Expected object key, got {:?}", other),
                    };

                    // ACCEPT BOTH ':' AND '=' HERE
                    match &self.current {
                        Token::Colon | Token::Equals => self.advance(),
                        other => panic!("Expected ':' or '=' after object key, got {:?}", other),
                    }

                    let value = self.parse_value();
                    entries.push((key, value));

                    // delimiter: comma is optional
                    match &self.current {
                        Token::Comma => {
                            self.advance();
                            continue;
                        }

                        // end object
                        Token::RBrace => {
                            self.advance();
                            break;
                        }

                        // LINE-BASED: next key starts immediately (no comma)
                        Token::Ident(_) | Token::String(_) => {
                            continue;
                        }

                        other => panic!(
                            "Expected ',', '}}', or next key in object literal, got {:?}",
                            other
                        ),
                    }
                }

                SceneValue::Object(Cow::Owned(
                    entries
                        .into_iter()
                        .map(|(k, v)| (Cow::Owned(k), v))
                        .collect(),
                ))
            }
            Token::LBracket => {
                self.advance();
                let mut items = Vec::new();
                loop {
                    if self.current == Token::RBracket {
                        self.advance();
                        break;
                    }

                    let value = self.parse_value();
                    items.push(value);

                    match &self.current {
                        Token::Comma => {
                            self.advance();
                            continue;
                        }
                        Token::RBracket => {
                            self.advance();
                            break;
                        }
                        _ => {}
                    }
                }
                SceneValue::Array(Cow::Owned(items))
            }

            _ => panic!("Invalid value token {:?}", self.current),
        }
    }

    fn parse_type_block_after_lbracket(&mut self) -> SceneNodeData {
        let ty = self.expect_ident();
        self.expect(Token::RBracket);

        let mut fields = Vec::new();
        let mut base = None;

        loop {
            match &self.current {
                Token::LBracket => {
                    self.advance();
                    if self.current == Token::Slash {
                        self.advance();
                        let end = self.expect_ident();
                        self.expect(Token::RBracket);
                        assert_eq!(end, ty);
                        break;
                    } else {
                        let nested = self.parse_type_block_after_lbracket();
                        base = Some(SceneNodeDataBase::Owned(Box::new(nested)));
                    }
                }

                Token::Ident(_) => {
                    let key = self.expect_ident();
                    self.expect(Token::Equals);
                    let val = self.parse_value();
                    fields.push((Cow::Owned(key), val));
                }

                _ => self.advance(),
            }
        }

        normalize_node_fields_for_type(&ty, &mut fields);
        SceneNodeData {
            ty: Cow::Owned(ty.clone()),
            fields: Cow::Owned(fields),
            base,
        }
    }

    fn parse_tags(&mut self) -> Vec<String> {
        self.expect(Token::LBracket);
        let mut tags = Vec::new();

        loop {
            match &self.current {
                Token::RBracket => {
                    self.advance();
                    break;
                }
                Token::String(s) | Token::Ident(s) => {
                    tags.push(s.clone());
                    self.advance();
                }
                Token::At => {
                    self.advance();
                    let name = self.expect_ident();
                    let resolved = self
                        .vars
                        .get(&name)
                        .cloned()
                        .unwrap_or_else(|| panic!("Unknown variable @{name}"));
                    match resolved {
                        SceneValue::Str(tag) => tags.push(tag.to_string()),
                        SceneValue::Key(key) => tags.push(key.to_string()),
                        _ => panic!("tags variable @{name} must resolve to a string or key"),
                    }
                }
                other => {
                    panic!("tags entries must be strings, identifiers, or @vars; got {other:?}")
                }
            }

            if self.current == Token::Comma {
                self.advance();
                continue;
            }
            if self.current == Token::RBracket {
                self.advance();
                break;
            }
            panic!("Expected ',' or ']' in tags list, got {:?}", self.current);
        }

        tags
    }

    fn parse_scene_inner(mut self) -> Scene {
        let mut nodes = Vec::new();
        let mut root = None;

        while self.current != Token::Eof {
            match self.current {
                Token::At => {
                    self.advance();
                    let name = self.expect_ident();
                    self.expect(Token::Equals);

                    if name == "root" {
                        match &self.current {
                            Token::Ident(key) => {
                                let k = key.clone();
                                self.advance();
                                root = Some(SceneKey::from(k.clone()));
                                self.vars.insert(
                                    "root".to_string(),
                                    SceneValue::Key(SceneValueKey::from(k)),
                                );
                            }
                            _ => panic!("root must be a scene key"),
                        }
                    } else {
                        let value = self.parse_value();
                        self.vars.insert(name, value);
                    }
                }

                Token::LBracket => {
                    self.advance();
                    let key = self.expect_ident();
                    self.expect(Token::RBracket);

                    let mut name = None;
                    let mut tags = Vec::new();
                    let mut parent = None;
                    let mut script = None;
                    let mut clear_script = false;
                    let mut root_of = None;
                    let mut script_vars: Vec<SceneObjectField> = Vec::new();

                    while matches!(self.current, Token::Ident(_)) {
                        let k = self.expect_ident();
                        self.expect(Token::Equals);
                        if k == "tags" {
                            tags = self.parse_tags();
                            continue;
                        }
                        let v = self.parse_value();
                        match k.as_ref() {
                            "name" => {
                                name = Some(match v {
                                    SceneValue::Str(s) => s.to_string(),
                                    _ => panic!("name must be a string"),
                                })
                            }
                            "parent" => {
                                parent = Some(match v {
                                    SceneValue::Key(k) => k.to_string(),
                                    _ => panic!("parent must be a key"),
                                })
                            }
                            "script" => match v {
                                SceneValue::Str(s) => {
                                    script = Some(s.to_string());
                                    clear_script = false;
                                }
                                SceneValue::Key(k) if k.as_ref() == "null" => {
                                    script = None;
                                    clear_script = true;
                                }
                                _ => panic!("script must be a string or null"),
                            },
                            "clear_script" => {
                                clear_script = match v {
                                    SceneValue::Bool(v) => v,
                                    _ => panic!("clear_script must be a bool"),
                                };
                            }
                            "root_of" => {
                                root_of = Some(match v {
                                    SceneValue::Str(s) => s.to_string(),
                                    _ => panic!("root_of must be a string"),
                                })
                            }
                            "script_vars" => match v {
                                SceneValue::Object(entries) => {
                                    script_vars = entries
                                        .iter()
                                        .map(|(k, v)| (Cow::Owned(k.to_string()), v.clone()))
                                        .collect();
                                }
                                _ => panic!("script_vars must be an object"),
                            },
                            _ => {}
                        }
                    }

                    if self.current != Token::LBracket {
                        panic!(
                            "Expected node type block or closing tag for node `{key}`, got {:?}",
                            self.current
                        );
                    }
                    self.advance();

                    let (data, has_data_override) = if self.current == Token::Slash {
                        if root_of.is_none() {
                            panic!("Node `{key}` must define a type block unless it uses root_of");
                        }
                        (
                            SceneNodeData {
                                ty: Cow::Borrowed("Node"),
                                fields: Cow::Owned(Vec::new()),
                                base: None,
                            },
                            false,
                        )
                    } else {
                        (self.parse_type_block_after_lbracket(), true)
                    };

                    if has_data_override {
                        self.expect(Token::LBracket);
                        self.expect(Token::Slash);
                        let end = self.expect_ident();
                        self.expect(Token::RBracket);
                        assert_eq!(end, key);
                    } else {
                        self.expect(Token::Slash);
                        let end = self.expect_ident();
                        self.expect(Token::RBracket);
                        assert_eq!(end, key);
                    }

                    let name = name.or_else(|| Some(key.clone()));

                    nodes.push(SceneNodeEntry {
                        has_data_override,
                        key: SceneKey::from(key),
                        name: name.map(Cow::Owned),
                        tags: Cow::Owned(tags.into_iter().map(Cow::Owned).collect()),
                        children: Cow::Owned(Vec::new()),
                        parent: parent.map(SceneKey::from),
                        script: script.map(Cow::Owned),
                        script_hash: None,
                        clear_script,
                        root_of: root_of.map(Cow::Owned),
                        script_vars: Cow::Owned(script_vars),
                        data,
                    });
                }

                _ => self.advance(),
            }
        }

        Scene {
            nodes: Cow::Owned(nodes),
            root,
        }
    }

    pub fn parse_scene(self) -> Scene {
        let vars = Parser::new(self.src).collect_vars();
        let mut parser = Parser::new(self.src);
        parser.vars = vars;
        parser.parse_scene_inner()
    }

    pub fn parse_value_literal(mut self) -> SceneValue {
        let value = self.parse_value();
        if self.current != Token::Eof {
            panic!("Expected end of value, got {:?}", self.current);
        }
        value
    }
}

fn normalize_node_fields_for_type(ty: &str, fields: &mut Vec<SceneObjectField>) {
    if ty != "Node3D" {
        return;
    }

    let mut rotation_present = false;
    let mut rotation_deg_xyz = None;

    for (name, value) in fields.iter_mut() {
        if name.as_ref() == "rotation" {
            rotation_present = true;
            if let SceneValue::Vec3 { x, y, z } = value.clone() {
                *value = euler_xyz_radians_to_quat_value(x, y, z);
            }
            continue;
        }

        if name.as_ref() == "rotation_deg" {
            if let SceneValue::Vec3 { x, y, z } = value.clone() {
                rotation_deg_xyz = Some((x, y, z));
            }
            continue;
        }
    }

    if !rotation_present && let Some((x_deg, y_deg, z_deg)) = rotation_deg_xyz {
        fields.push((
            Cow::Borrowed("rotation"),
            euler_xyz_radians_to_quat_value(
                x_deg.to_radians(),
                y_deg.to_radians(),
                z_deg.to_radians(),
            ),
        ));
    }

    fields.retain(|(name, _)| name.as_ref() != "rotation_deg");
}

fn euler_xyz_radians_to_quat_value(x: f32, y: f32, z: f32) -> SceneValue {
    let mut rotation = Quaternion::IDENTITY;
    rotation.rotate_xyz(x, y, z);
    SceneValue::Vec4 {
        x: rotation.x,
        y: rotation.y,
        z: rotation.z,
        w: rotation.w,
    }
}
