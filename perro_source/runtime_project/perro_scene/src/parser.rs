// parser.rs - Parse into scene types
use crate::{
    Lexer, Scene, SceneFieldName, SceneKey, SceneNodeData, SceneNodeDataBase, SceneNodeEntry,
    SceneObjectField, SceneValue, SceneValueKey, Token,
};
use perro_nodes::NodeType;
use perro_structs::Quaternion;
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::str::FromStr;

pub struct Parser<'a> {
    src: &'a str,
    lexer: Lexer<'a>,
    current: Token<'a>,
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

    fn expect(&mut self, t: Token<'a>) {
        if self.current != t {
            panic!("Expected {:?}, got {:?}", t, self.current);
        }
        self.advance();
    }

    fn expect_ident(&mut self) -> &'a str {
        match std::mem::replace(&mut self.current, Token::Eof) {
            Token::Ident(s) => {
                self.advance();
                s
            }
            other => panic!("Expected identifier, got {:?}", other),
        }
    }

    fn expect_scene_key(&mut self) -> Cow<'a, str> {
        let mut at_count = 0;
        while self.current == Token::At {
            self.advance();
            at_count += 1;
        }
        let ident = self.expect_ident();
        if at_count == 0 {
            return Cow::Borrowed(ident);
        }
        let mut key = String::with_capacity(at_count + ident.len());
        key.extend(std::iter::repeat_n('@', at_count));
        key.push_str(ident);
        Cow::Owned(key)
    }

    fn expect_node_ref_key(&mut self) -> String {
        let mut at_count = 0;
        while self.current == Token::At {
            self.advance();
            at_count += 1;
        }
        let ident = self.expect_ident();
        if at_count == 0 {
            return ident.to_string();
        }
        let mut key = String::with_capacity(at_count + ident.len());
        key.extend(std::iter::repeat_n('@', at_count));
        key.push_str(ident);
        key
    }

    fn collect_vars(self) -> HashMap<String, SceneValue> {
        self.collect_var_entries().into_iter().collect()
    }

    pub(crate) fn collect_var_entries(mut self) -> Vec<(String, SceneValue)> {
        let mut vars = Vec::new();
        while self.current != Token::Eof {
            if self.current == Token::Dollar {
                self.advance();
                let name = self.expect_ident().to_string();
                if self.current == Token::Equals {
                    self.advance();
                    let value = self.parse_value();
                    self.vars.insert(name.clone(), value.clone());
                    vars.push((name, value));
                }
                continue;
            }
            if self.current == Token::At {
                self.advance();
                if self.current == Token::At {
                    continue;
                }
                let name = self.expect_ident();
                if name == "root" && self.current == Token::Equals {
                    self.advance();
                    let value = self.parse_value();
                    self.vars.insert("root".to_string(), value.clone());
                    vars.push(("root".to_string(), value));
                }
                continue;
            }
            self.advance();
        }
        vars
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

            Token::Dollar => {
                self.advance();
                let name = self.expect_ident();
                self.vars
                    .get(name)
                    .cloned()
                    .unwrap_or_else(|| panic!("Unknown variable ${name}"))
            }

            Token::Percent => {
                self.advance();
                let marker = self.expect_ident();
                self.expect(Token::Colon);
                if marker != "loc" {
                    panic!("Unknown percent marker %{marker}:");
                }
                let key = match std::mem::replace(&mut self.current, Token::Eof) {
                    Token::String(s) => {
                        self.advance();
                        s
                    }
                    Token::Ident(s) => {
                        self.advance();
                        s.to_string()
                    }
                    other => panic!("Expected locale key after %loc:, got {:?}", other),
                };
                SceneValue::Str(Cow::Owned(format!("%loc:{key}")))
            }

            Token::At => {
                self.advance();
                let key = self.expect_node_ref_key();
                SceneValue::Key(SceneValueKey::from(key))
            }

            Token::Ident(name) => {
                let key = (*name).to_string();
                self.advance();
                if matches!(key.as_str(), "only" | "without") && self.current == Token::LParen {
                    return self.parse_bitmask_call_key(key);
                }
                SceneValue::Key(SceneValueKey::from(key))
            }

            Token::LParen => {
                self.advance();
                let mut nums = [0.0; 4];
                let mut len = 0;
                loop {
                    if let Token::Number(n) = self.current {
                        if len >= nums.len() {
                            panic!("Invalid vector length");
                        }
                        nums[len] = n;
                        len += 1;
                        self.advance();
                    }
                    if self.current == Token::Comma {
                        self.advance();
                        continue;
                    }
                    break;
                }
                self.expect(Token::RParen);

                match len {
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
                            let out = name.to_string();
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
                        .map(|(k, v)| (SceneFieldName::from(k), v))
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

    fn parse_bitmask_call_key(&mut self, name: String) -> SceneValue {
        self.expect(Token::LParen);
        let mut layers = Vec::new();
        let mut bracketed = false;

        if self.current == Token::LBracket {
            bracketed = true;
            self.advance();
        }

        loop {
            match self.current {
                Token::Number(n) => {
                    if n.fract() != 0.0 || !(1.0..=32.0).contains(&n) {
                        panic!("BitMask layer must be 1..=32");
                    }
                    layers.push(n as u8);
                    self.advance();
                }
                Token::RBracket if bracketed => {
                    self.advance();
                    break;
                }
                Token::RParen if !bracketed => break,
                ref other => panic!("Expected BitMask layer, got {:?}", other),
            }

            match self.current {
                Token::Comma => {
                    self.advance();
                }
                Token::RBracket if bracketed => {
                    self.advance();
                    break;
                }
                Token::RParen if !bracketed => break,
                ref other => panic!("Expected ',' or ')' in BitMask call, got {:?}", other),
            }
        }

        self.expect(Token::RParen);

        let mut out = String::new();
        out.push_str(&name);
        out.push('(');
        for (idx, layer) in layers.iter().enumerate() {
            if idx > 0 {
                out.push_str(", ");
            }
            out.push_str(&layer.to_string());
        }
        out.push(')');
        SceneValue::Key(SceneValueKey::from(out))
    }

    fn parse_type_block_after_lbracket(&mut self) -> SceneNodeData {
        let ty = self.expect_ident().to_string();
        if self.current == Token::Slash {
            self.advance();
            self.expect(Token::RBracket);
            return SceneNodeData {
                ty: canonical_node_type_name(&ty),
                fields: Cow::Owned(Vec::new()),
                base: None,
            };
        }
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
                    fields.push((canonical_scene_field_name(key), val));
                }

                _ => self.advance(),
            }
        }

        normalize_node_fields_for_type(&ty, &mut fields);
        SceneNodeData {
            ty: canonical_node_type_name(&ty),
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
                Token::String(s) => {
                    tags.push(s.clone());
                    self.advance();
                }
                Token::Ident(s) => {
                    tags.push(s.to_string());
                    self.advance();
                }
                Token::Dollar => {
                    self.advance();
                    let name = self.expect_ident().to_string();
                    let resolved = self
                        .vars
                        .get(&name)
                        .cloned()
                        .unwrap_or_else(|| panic!("Unknown variable ${name}"));
                    match resolved {
                        SceneValue::Str(tag) => tags.push(tag.to_string()),
                        SceneValue::Key(key) => tags.push(key.to_string()),
                        _ => panic!("tags variable ${name} must resolve to a string or key"),
                    }
                }
                other => {
                    panic!("tags entries must be strings, identifiers, or $vars; got {other:?}")
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
        let mut root_name = None::<String>;
        let mut key_names = Vec::<Cow<'static, str>>::new();
        let mut key_ids = HashMap::<Cow<'a, str>, SceneKey>::new();
        let mut defined_keys = HashSet::<SceneKey>::new();
        let mut pending_parents = Vec::<(usize, String)>::new();

        while self.current != Token::Eof {
            match self.current {
                Token::Dollar => {
                    self.advance();
                    let name = self.expect_ident();
                    self.expect(Token::Equals);

                    if name == "root" {
                        match self.parse_value() {
                            SceneValue::Key(k) => {
                                let key = k.to_string();
                                root_name = Some(key.clone());
                                self.vars.insert("root".to_string(), SceneValue::Key(k));
                            }
                            _ => panic!("root must be a node ref like @Main"),
                        }
                    } else {
                        let value = self.parse_value();
                        self.vars.insert(name.to_string(), value);
                    }
                }

                Token::At => {
                    self.advance();
                    let name = self.expect_ident();
                    self.expect(Token::Equals);

                    if name == "root" {
                        match self.parse_value() {
                            SceneValue::Key(k) => {
                                let key = k.to_string();
                                root_name = Some(key.clone());
                                self.vars.insert("root".to_string(), SceneValue::Key(k));
                            }
                            _ => panic!("root must be a node ref like @Main"),
                        }
                    } else {
                        let _ = self.parse_value();
                    }
                }

                Token::LBracket => {
                    self.advance();
                    let key = self.expect_scene_key();
                    self.expect(Token::RBracket);
                    let key_ref = key.as_ref();
                    let key_id = if let Some(key_id) = key_ids.get(key_ref) {
                        *key_id
                    } else {
                        let key_id = SceneKey::new(key_names.len() as u32);
                        key_ids.insert(key.clone(), key_id);
                        key_names.push(Cow::Owned(key_ref.to_string()));
                        key_id
                    };
                    if !defined_keys.insert(key_id) {
                        panic!("duplicate scene key `{key_ref}`");
                    }

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
                        match k {
                            "name" => {
                                name = Some(match v {
                                    SceneValue::Str(s) => s.into_owned(),
                                    _ => panic!("name must be a string"),
                                })
                            }
                            "parent" => {
                                parent = Some(match v {
                                    SceneValue::Key(k) => k.0.into_owned(),
                                    _ => panic!("parent must be a node ref like @Parent"),
                                })
                            }
                            "script" => match v {
                                SceneValue::Str(s) => {
                                    script = Some(s.into_owned());
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
                                    SceneValue::Str(s) => s.into_owned(),
                                    _ => panic!("root_of must be a string"),
                                })
                            }
                            "script_vars" => match v {
                                SceneValue::Object(entries) => {
                                    script_vars = entries.into_owned().into_iter().collect();
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
                        let end = self.expect_scene_key();
                        self.expect(Token::RBracket);
                        assert_eq!(end, key);
                    } else {
                        self.expect(Token::Slash);
                        let end = self.expect_scene_key();
                        self.expect(Token::RBracket);
                        assert_eq!(end, key);
                    }

                    let name = name.or_else(|| Some(key_ref.to_string()));
                    let parent_name = parent;

                    nodes.push(SceneNodeEntry {
                        has_data_override,
                        key: key_id,
                        name: name.map(Cow::Owned),
                        tags: Cow::Owned(tags.into_iter().map(Cow::Owned).collect()),
                        children: Cow::Owned(Vec::new()),
                        parent: None,
                        script: script.map(Cow::Owned),
                        clear_script,
                        root_of: root_of.map(Cow::Owned),
                        script_vars: Cow::Owned(script_vars),
                        data,
                    });
                    if let Some(parent_name) = parent_name {
                        pending_parents.push((nodes.len() - 1, parent_name));
                    }
                }

                _ => self.advance(),
            }
        }

        for (idx, parent_name) in pending_parents {
            let parent = if let Some(parent) = key_ids.get(parent_name.as_str()) {
                *parent
            } else {
                let parent = SceneKey::new(key_names.len() as u32);
                key_ids.insert(Cow::Owned(parent_name.clone()), parent);
                key_names.push(Cow::Owned(parent_name));
                parent
            };
            nodes[idx].parent = Some(parent);
        }
        let root = root_name.map(|name| {
            if let Some(root) = key_ids.get(name.as_str()) {
                *root
            } else {
                let root = SceneKey::new(key_names.len() as u32);
                key_ids.insert(Cow::Owned(name.clone()), root);
                key_names.push(Cow::Owned(name));
                root
            }
        });

        Scene {
            nodes: Cow::Owned(nodes),
            root,
            key_names: Cow::Owned(key_names),
        }
    }

    pub fn parse_scene(self) -> Scene {
        let mut parser = Parser::new(self.src);
        if needs_var_prefetch(self.src) {
            parser.vars = Parser::new(self.src).collect_vars();
        }
        parser.parse_scene_inner()
    }

    pub fn parse_scene_doc(self) -> crate::SceneDoc {
        crate::SceneDoc::parse(self.src)
    }

    pub fn parse_value_literal(mut self) -> SceneValue {
        let value = self.parse_value();
        if self.current != Token::Eof {
            panic!("Expected end of value, got {:?}", self.current);
        }
        value
    }
}

fn needs_var_prefetch(src: &str) -> bool {
    let bytes = src.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'$' {
            i += 1;
            continue;
        }
        i += 1;
        let start = i;
        while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
            i += 1;
        }
        if start != i && &src[start..i] != "root" {
            return true;
        }
    }
    false
}

fn canonical_node_type_name(name: &str) -> Cow<'static, str> {
    match name {
        "Node" => Cow::Borrowed("Node"),
        "Node2D" => Cow::Borrowed("Node2D"),
        "Sprite2D" => Cow::Borrowed("Sprite2D"),
        "AnimatedSprite2D" => Cow::Borrowed("AnimatedSprite2D"),
        "ParticleEmitter2D" => Cow::Borrowed("ParticleEmitter2D"),
        "AmbientLight2D" => Cow::Borrowed("AmbientLight2D"),
        "RayLight2D" => Cow::Borrowed("RayLight2D"),
        "PointLight2D" => Cow::Borrowed("PointLight2D"),
        "SpotLight2D" => Cow::Borrowed("SpotLight2D"),
        "TileMap2D" => Cow::Borrowed("TileMap2D"),
        "Skeleton2D" => Cow::Borrowed("Skeleton2D"),
        "BoneAttachment2D" => Cow::Borrowed("BoneAttachment2D"),
        "IKTarget2D" => Cow::Borrowed("IKTarget2D"),
        "PhysicsBoneChain2D" => Cow::Borrowed("PhysicsBoneChain2D"),
        "BoneCollider2D" => Cow::Borrowed("BoneCollider2D"),
        "Camera2D" => Cow::Borrowed("Camera2D"),
        "CollisionShape2D" => Cow::Borrowed("CollisionShape2D"),
        "StaticBody2D" => Cow::Borrowed("StaticBody2D"),
        "Area2D" => Cow::Borrowed("Area2D"),
        "RigidBody2D" => Cow::Borrowed("RigidBody2D"),
        "PinJoint2D" => Cow::Borrowed("PinJoint2D"),
        "DistanceJoint2D" => Cow::Borrowed("DistanceJoint2D"),
        "FixedJoint2D" => Cow::Borrowed("FixedJoint2D"),
        "AudioMask2D" => Cow::Borrowed("AudioMask2D"),
        "AudioEffectZone2D" => Cow::Borrowed("AudioEffectZone2D"),
        "AudioPortal2D" => Cow::Borrowed("AudioPortal2D"),
        "Node3D" => Cow::Borrowed("Node3D"),
        "MeshInstance3D" => Cow::Borrowed("MeshInstance3D"),
        "MultiMeshInstance3D" => Cow::Borrowed("MultiMeshInstance3D"),
        "CollisionShape3D" => Cow::Borrowed("CollisionShape3D"),
        "StaticBody3D" => Cow::Borrowed("StaticBody3D"),
        "Area3D" => Cow::Borrowed("Area3D"),
        "RigidBody3D" => Cow::Borrowed("RigidBody3D"),
        "BallJoint3D" => Cow::Borrowed("BallJoint3D"),
        "HingeJoint3D" => Cow::Borrowed("HingeJoint3D"),
        "FixedJoint3D" => Cow::Borrowed("FixedJoint3D"),
        "AudioMask3D" => Cow::Borrowed("AudioMask3D"),
        "AudioEffectZone3D" => Cow::Borrowed("AudioEffectZone3D"),
        "AudioPortal3D" => Cow::Borrowed("AudioPortal3D"),
        "Skeleton3D" => Cow::Borrowed("Skeleton3D"),
        "BoneAttachment3D" => Cow::Borrowed("BoneAttachment3D"),
        "IKTarget3D" => Cow::Borrowed("IKTarget3D"),
        "PhysicsBoneChain3D" => Cow::Borrowed("PhysicsBoneChain3D"),
        "BoneCollider3D" => Cow::Borrowed("BoneCollider3D"),
        "Camera3D" => Cow::Borrowed("Camera3D"),
        "ParticleEmitter3D" => Cow::Borrowed("ParticleEmitter3D"),
        "AnimationPlayer" => Cow::Borrowed("AnimationPlayer"),
        "AnimationTree" => Cow::Borrowed("AnimationTree"),
        "AmbientLight3D" => Cow::Borrowed("AmbientLight3D"),
        "Sky3D" => Cow::Borrowed("Sky3D"),
        "RayLight3D" => Cow::Borrowed("RayLight3D"),
        "PointLight3D" => Cow::Borrowed("PointLight3D"),
        "SpotLight3D" => Cow::Borrowed("SpotLight3D"),
        "UiBox" => Cow::Borrowed("UiBox"),
        "UiPanel" => Cow::Borrowed("UiPanel"),
        "UiButton" => Cow::Borrowed("UiButton"),
        "UiImage" => Cow::Borrowed("UiImage"),
        "UiAnimatedImage" => Cow::Borrowed("UiAnimatedImage"),
        "UiLabel" => Cow::Borrowed("UiLabel"),
        "UiTextBox" => Cow::Borrowed("UiTextBox"),
        "UiTextBlock" => Cow::Borrowed("UiTextBlock"),
        "UiScrollContainer" => Cow::Borrowed("UiScrollContainer"),
        "UiScroll" => Cow::Borrowed("UiScroll"),
        "UiLayout" => Cow::Borrowed("UiLayout"),
        "UiHLayout" => Cow::Borrowed("UiHLayout"),
        "UiHBox" => Cow::Borrowed("UiHBox"),
        "UiVLayout" => Cow::Borrowed("UiVLayout"),
        "UiVBox" => Cow::Borrowed("UiVBox"),
        "UiGrid" => Cow::Borrowed("UiGrid"),
        "UiTreeList" => Cow::Borrowed("UiTreeList"),
        other => Cow::Owned(other.to_string()),
    }
}

fn canonical_scene_field_name(name: &str) -> SceneFieldName {
    SceneFieldName::from_borrowed(name).unwrap_or_else(|| SceneFieldName::from(name.to_string()))
}

fn normalize_node_fields_for_type(ty: &str, fields: &mut Vec<SceneObjectField>) {
    let Ok(node_type) = NodeType::from_str(ty) else {
        return;
    };
    let is_2d = node_type.is_a(NodeType::Node2D) || node_type.is_a(NodeType::UiBox);
    let is_3d = node_type.is_a(NodeType::Node3D);
    if !is_2d && !is_3d {
        return;
    }

    let mut rotation_present = false;
    let mut rotation_deg_2d = None;
    let mut rotation_deg_3d = None;

    for (name, value) in fields.iter_mut() {
        if name.as_ref() == "rotation" {
            rotation_present = true;
            if is_3d && let SceneValue::Vec3 { x, y, z } = value.clone() {
                *value = euler_xyz_radians_to_quat_value(x, y, z);
            }
            continue;
        }

        if name.as_ref() == "rotation_deg" {
            if is_2d && let Some(v) = value.as_f32() {
                rotation_deg_2d = Some(v);
            }
            if is_3d && let SceneValue::Vec3 { x, y, z } = value.clone() {
                rotation_deg_3d = Some((x, y, z));
            }
            continue;
        }
    }

    if !rotation_present
        && is_2d
        && let Some(deg) = rotation_deg_2d
    {
        fields.push((SceneFieldName::Rotation, SceneValue::F32(deg.to_radians())));
    }

    if !rotation_present
        && is_3d
        && let Some((x_deg, y_deg, z_deg)) = rotation_deg_3d
    {
        fields.push((
            SceneFieldName::Rotation,
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

#[cfg(test)]
mod tests {
    use super::Parser;

    #[test]
    fn parser_keeps_script_path_string() {
        let src = "$root = main\n\n[main]\nscript = \"dlc://test/scripts/script.rs\"\n[/main]\n";
        let scene = Parser::new(src).parse_scene();
        let node = &scene.nodes[0];
        assert_eq!(node.script.as_deref(), Some("dlc://test/scripts/script.rs"));
    }

    #[test]
    fn parser_keeps_root_of_path_string() {
        let src = "$root = main\n\n[main]\nroot_of = \"dlc://test/scenes/main.scn\"\n[/main]\n";
        let scene = Parser::new(src).parse_scene();
        let node = &scene.nodes[0];
        assert_eq!(node.root_of.as_deref(), Some("dlc://test/scenes/main.scn"));
    }
}
