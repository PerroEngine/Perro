// parser.rs - Parse into scene types
use crate::{
    Lexer, NodeFieldType, Scene, SceneFieldName, SceneKey, SceneNodeData, SceneNodeDataBase,
    SceneNodeEntry, SceneObjectField, SceneValue, SceneValueKey, Token, scene_node_spec,
};
use perro_nodes::NodeType;
use perro_structs::Quaternion;
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::str::FromStr;

type ParseResult<T> = Result<T, String>;

const MAX_SCENE_VALUE_DEPTH: usize = 128;

pub struct Parser<'a> {
    src: &'a str,
    lexer: Lexer<'a>,
    current: Token<'a>,
    vars: HashMap<String, SceneValue>,
    lenient_separators: bool,
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
            lenient_separators: false,
        }
    }

    pub(crate) fn new_lenient(src: &'a str) -> Self {
        let mut parser = Self::new(src);
        parser.lenient_separators = true;
        parser
    }

    fn advance(&mut self) {
        self.current = self.lexer.next_token();
    }

    fn expect(&mut self, t: Token<'a>) -> ParseResult<()> {
        if self.current != t {
            return Err(format!("Expected {:?}, got {:?}", t, self.current));
        }
        self.advance();
        Ok(())
    }

    fn expect_ident(&mut self) -> ParseResult<&'a str> {
        match std::mem::replace(&mut self.current, Token::Eof) {
            Token::Ident(s) => {
                self.advance();
                Ok(s)
            }
            other => Err(format!("Expected identifier, got {:?}", other)),
        }
    }

    fn expect_scene_key(&mut self) -> ParseResult<Cow<'a, str>> {
        let mut at_count = 0;
        while self.current == Token::At {
            self.advance();
            at_count += 1;
        }
        let ident = self.expect_ident()?;
        if at_count == 0 {
            return Ok(Cow::Borrowed(ident));
        }
        let mut key = String::with_capacity(at_count + ident.len());
        key.extend(std::iter::repeat_n('@', at_count));
        key.push_str(ident);
        Ok(Cow::Owned(key))
    }

    fn expect_node_ref_key(&mut self) -> ParseResult<String> {
        let mut at_count = 0;
        while self.current == Token::At {
            self.advance();
            at_count += 1;
        }
        let ident = self.expect_ident()?;
        if at_count == 0 {
            return Ok(ident.to_string());
        }
        let mut key = String::with_capacity(at_count + ident.len());
        key.extend(std::iter::repeat_n('@', at_count));
        key.push_str(ident);
        Ok(key)
    }

    fn try_collect_vars(mut self) -> ParseResult<HashMap<String, SceneValue>> {
        Ok(self.try_collect_var_entries()?.into_iter().collect())
    }

    pub(crate) fn collect_var_entries(mut self) -> Vec<(String, SceneValue)> {
        self.try_collect_var_entries()
            .unwrap_or_else(|err| panic!("{err}"))
    }

    pub(crate) fn try_collect_var_entries(&mut self) -> ParseResult<Vec<(String, SceneValue)>> {
        let mut vars = Vec::new();
        while self.current != Token::Eof {
            if let Token::Error(err) = &self.current {
                return Err(err.to_string());
            }
            if self.current == Token::Dollar {
                self.advance();
                let name = self.expect_ident()?.to_string();
                if self.current == Token::Equals {
                    self.advance();
                    let value = self.parse_value()?;
                    self.vars.insert(name.clone(), value.clone());
                    vars.push((name, value));
                }
                continue;
            }
            self.advance();
        }
        Ok(vars)
    }

    fn parse_value(&mut self) -> ParseResult<SceneValue> {
        self.parse_value_at_depth(0)
    }

    fn parse_value_at_depth(&mut self, depth: usize) -> ParseResult<SceneValue> {
        if depth > MAX_SCENE_VALUE_DEPTH {
            return Err(format!(
                "Scene value nesting exceeds limit of {MAX_SCENE_VALUE_DEPTH}"
            ));
        }

        match &self.current {
            Token::Number(n) => {
                let v = *n;
                self.advance();
                Ok(SceneValue::F32(v))
            }

            Token::String(s) => {
                let v = s.clone();
                self.advance();
                Ok(SceneValue::Str(Cow::Owned(v)))
            }

            Token::Dollar => {
                self.advance();
                let name = self.expect_ident()?;
                let value = self
                    .vars
                    .get(name)
                    .ok_or_else(|| format!("Unknown variable ${name}"))?;
                ensure_value_fits_depth(value, depth)?;
                Ok(value.clone())
            }

            Token::Percent => {
                self.advance();
                let marker = self.expect_ident()?;
                self.expect(Token::Colon)?;
                if marker != "loc" {
                    return Err(format!("Unknown percent marker %{marker}:"));
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
                    other => {
                        return Err(format!("Expected locale key after %loc:, got {:?}", other));
                    }
                };
                Ok(SceneValue::Str(Cow::Owned(format!("%loc:{key}"))))
            }

            Token::At => {
                self.advance();
                let key = self.expect_node_ref_key()?;
                Ok(SceneValue::Key(SceneValueKey::from(key)))
            }

            Token::Ident(name) => {
                let key = (*name).to_string();
                self.advance();
                if matches!(key.as_str(), "only" | "without") && self.current == Token::LParen {
                    return self.parse_bitmask_call_key(key);
                }
                Ok(SceneValue::Key(SceneValueKey::from(key)))
            }

            Token::LParen => {
                self.advance();
                let mut nums = [0.0; 4];
                let mut len = 0;
                loop {
                    if let Token::Number(n) = self.current {
                        if len >= nums.len() {
                            return Err("Invalid vector length".to_string());
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
                self.expect(Token::RParen)?;

                Ok(match len {
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
                    _ => return Err("Invalid vector length".to_string()),
                })
            }

            Token::True => {
                self.advance();
                Ok(SceneValue::Bool(true))
            }

            Token::False => {
                self.advance();
                Ok(SceneValue::Bool(false))
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
                        other => return Err(format!("Expected object key, got {:?}", other)),
                    };

                    // ACCEPT BOTH ':' AND '=' HERE
                    match &self.current {
                        Token::Colon | Token::Equals => self.advance(),
                        other => {
                            return Err(format!(
                                "Expected ':' or '=' after object key, got {:?}",
                                other
                            ));
                        }
                    }

                    let value = self.parse_value_at_depth(depth + 1)?;
                    entries.push((key, value));

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

                        Token::Ident(_) | Token::String(_) | Token::Number(_) => continue,

                        other => {
                            return Err(format!(
                                "Expected ',' or '}}' in object literal, got {:?}",
                                other
                            ));
                        }
                    }
                }

                Ok(SceneValue::Object(Cow::Owned(
                    entries
                        .into_iter()
                        .map(|(k, v)| (SceneFieldName::from(k), v))
                        .collect(),
                )))
            }
            Token::LBracket => {
                self.advance();
                let mut items = Vec::new();
                loop {
                    if self.current == Token::RBracket {
                        self.advance();
                        break;
                    }

                    let value = self.parse_value_at_depth(depth + 1)?;
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
                        _ if self.lenient_separators => {}
                        other => {
                            return Err(format!(
                                "Expected ',' or ']' in array literal, got {:?}",
                                other
                            ));
                        }
                    }
                }
                Ok(SceneValue::Array(Cow::Owned(items)))
            }

            _ => Err(format!("Invalid value token {:?}", self.current)),
        }
    }

    fn parse_bitmask_call_key(&mut self, name: String) -> ParseResult<SceneValue> {
        self.expect(Token::LParen)?;
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
                        return Err("BitMask layer must be 1..=32".to_string());
                    }
                    layers.push(n as u8);
                    self.advance();
                }
                Token::RBracket if bracketed => {
                    self.advance();
                    break;
                }
                Token::RParen if !bracketed => break,
                ref other => return Err(format!("Expected BitMask layer, got {:?}", other)),
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
                ref other => {
                    return Err(format!(
                        "Expected ',' or ')' in BitMask call, got {:?}",
                        other
                    ));
                }
            }
        }

        self.expect(Token::RParen)?;

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
        Ok(SceneValue::Key(SceneValueKey::from(out)))
    }

    fn parse_type_block_after_lbracket(&mut self) -> ParseResult<SceneNodeData> {
        let ty = self.expect_ident()?.to_string();
        if self.current == Token::Slash {
            self.advance();
            self.expect(Token::RBracket)?;
            return scene_node_data_from_parts(&ty, Cow::Owned(Vec::new()), None);
        }
        self.expect(Token::RBracket)?;

        let mut fields = Vec::new();
        let mut base = None;

        loop {
            match &self.current {
                Token::LBracket => {
                    self.advance();
                    if self.current == Token::Slash {
                        self.advance();
                        let end = self.expect_ident()?;
                        self.expect(Token::RBracket)?;
                        if end != ty {
                            return Err(format!("Expected closing tag `/{ty}`, got `/{end}`"));
                        }
                        break;
                    } else {
                        let nested = self.parse_type_block_after_lbracket()?;
                        base = Some(SceneNodeDataBase::Owned(Box::new(nested)));
                    }
                }

                Token::Ident(_) => {
                    let key = self.expect_ident()?;
                    self.expect(Token::Equals)?;
                    let val = self.parse_value()?;
                    fields.push((canonical_scene_field_name(key), val));
                }

                Token::Eof => {
                    return Err(format!("Unterminated node type block `[{ty}]`"));
                }

                Token::Error(err) => return Err(err.to_string()),

                _ => self.advance(),
            }
        }

        scene_node_data_from_parts(&ty, Cow::Owned(fields), base)
    }

    fn parse_tags(&mut self) -> ParseResult<Vec<String>> {
        self.expect(Token::LBracket)?;
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
                    let name = self.expect_ident()?.to_string();
                    let resolved = self
                        .vars
                        .get(&name)
                        .cloned()
                        .ok_or_else(|| format!("Unknown variable ${name}"))?;
                    match resolved {
                        SceneValue::Str(tag) => tags.push(tag.to_string()),
                        SceneValue::Key(key) => tags.push(key.to_string()),
                        _ => {
                            return Err(format!(
                                "tags variable ${name} must resolve to a string or key"
                            ));
                        }
                    }
                }
                other => {
                    return Err(format!(
                        "tags entries must be strings, identifiers, or $vars; got {other:?}"
                    ));
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
            return Err(format!(
                "Expected ',' or ']' in tags list, got {:?}",
                self.current
            ));
        }

        Ok(tags)
    }

    fn skip_closing_tag_after_lbracket(&mut self) -> ParseResult<Cow<'a, str>> {
        self.expect(Token::Slash)?;
        let name = self.expect_scene_key()?;
        self.expect(Token::RBracket)?;
        Ok(name)
    }

    fn skip_node_body(&mut self, key: &str) -> ParseResult<()> {
        loop {
            match self.current {
                Token::Eof => break,
                Token::LBracket => {
                    self.advance();
                    if self.current == Token::Slash {
                        let end = self.skip_closing_tag_after_lbracket()?;
                        if end.as_ref() == key {
                            break;
                        }
                    }
                }
                _ => self.advance(),
            }
        }
        Ok(())
    }

    pub(crate) fn parse_scene_inner(mut self) -> ParseResult<Scene> {
        let mut nodes = Vec::new();
        let mut root_name = None::<String>;
        let mut key_names = Vec::<Cow<'static, str>>::new();
        let mut key_ids = HashMap::<Cow<'a, str>, SceneKey>::new();
        let mut defined_keys = HashSet::<SceneKey>::new();
        let mut pending_parents = Vec::<(usize, String)>::new();

        while self.current != Token::Eof {
            match self.current {
                Token::Error(ref err) => return Err(err.to_string()),
                Token::Dollar => {
                    self.advance();
                    let name = self.expect_ident()?;
                    self.expect(Token::Equals)?;

                    if name == "root" {
                        match self.parse_value()? {
                            SceneValue::Key(k) => {
                                let key = k.to_string();
                                root_name = Some(key.clone());
                                self.vars.insert("root".to_string(), SceneValue::Key(k));
                            }
                            _ => return Err("root must be a node ref like @Main".to_string()),
                        }
                    } else {
                        let value = self.parse_value()?;
                        self.vars.insert(name.to_string(), value);
                    }
                }

                Token::At => {
                    return Err("use `$root = @NodeKey`; @ only marks node refs".to_string());
                }

                Token::LBracket => {
                    self.advance();
                    if self.current == Token::Slash {
                        let end = self.skip_closing_tag_after_lbracket()?;
                        if self.lenient_separators {
                            continue;
                        }
                        return Err(format!(
                            "unexpected closing tag `/{end}` outside node block"
                        ));
                    }
                    let key = self.expect_scene_key()?;
                    self.expect(Token::RBracket)?;
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
                        if self.lenient_separators {
                            self.skip_node_body(key_ref)?;
                            continue;
                        }
                        return Err(format!("duplicate scene key `{key_ref}`"));
                    }

                    let mut name = None;
                    let mut tags = Vec::new();
                    let mut parent = None;
                    let mut script = None;
                    let mut clear_script = false;
                    let mut root_of = None;
                    let mut script_vars: Vec<SceneObjectField> = Vec::new();

                    while matches!(self.current, Token::Ident(_)) {
                        let k = self.expect_ident()?;
                        self.expect(Token::Equals)?;
                        if k == "tags" {
                            tags = self.parse_tags()?;
                            continue;
                        }
                        let v = self.parse_value()?;
                        match k {
                            "name" => {
                                name = Some(match v {
                                    SceneValue::Str(s) => s.into_owned(),
                                    _ => return Err("name must be a string".to_string()),
                                })
                            }
                            "parent" => {
                                parent = Some(match v {
                                    SceneValue::Key(k) => k.0.into_owned(),
                                    _ => {
                                        return Err(
                                            "parent must be a node ref like @Parent".to_string()
                                        );
                                    }
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
                                _ => return Err("script must be a string or null".to_string()),
                            },
                            "clear_script" => {
                                clear_script = match v {
                                    SceneValue::Bool(v) => v,
                                    _ => return Err("clear_script must be a bool".to_string()),
                                };
                            }
                            "root_of" => {
                                root_of = Some(match v {
                                    SceneValue::Str(s) => s.into_owned(),
                                    _ => return Err("root_of must be a string".to_string()),
                                })
                            }
                            "script_vars" => match v {
                                SceneValue::Object(entries) => {
                                    script_vars = custom_script_var_fields(entries.into_owned());
                                }
                                _ => return Err("script_vars must be an object".to_string()),
                            },
                            _ => {}
                        }
                    }

                    if self.current != Token::LBracket {
                        return Err(format!(
                            "Expected node type block or closing tag for node `{key}`, got {:?}",
                            self.current
                        ));
                    }
                    self.advance();

                    let (data, has_data_override) = if self.current == Token::Slash {
                        (
                            SceneNodeData::new(NodeType::Node, Cow::Owned(Vec::new()), None),
                            false,
                        )
                    } else {
                        (self.parse_type_block_after_lbracket()?, true)
                    };

                    if has_data_override {
                        self.expect(Token::LBracket)?;
                        self.expect(Token::Slash)?;
                        let end = self.expect_scene_key()?;
                        self.expect(Token::RBracket)?;
                        if end != key {
                            return Err(format!("Expected closing tag `/{}`, got `/{}`", key, end));
                        }
                    } else {
                        self.expect(Token::Slash)?;
                        let end = self.expect_scene_key()?;
                        self.expect(Token::RBracket)?;
                        if end != key {
                            return Err(format!("Expected closing tag `/{}`, got `/{}`", key, end));
                        }
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
                return Err(format!(
                    "parent node key `{}` not found for child `{}`",
                    parent_name,
                    key_names
                        .get(nodes[idx].key.as_usize())
                        .map(|name| name.as_ref())
                        .unwrap_or("<unknown>")
                ));
            };
            nodes[idx].parent = Some(parent);
        }
        let root = if let Some(name) = root_name {
            Some(
                *key_ids
                    .get(name.as_str())
                    .ok_or_else(|| format!("scene root `{name}` not found in node list"))?,
            )
        } else {
            None
        };

        Ok(Scene {
            nodes: Cow::Owned(nodes),
            root,
            key_names: Cow::Owned(key_names),
        })
    }

    pub fn parse_scene(self) -> Scene {
        self.try_parse_scene().unwrap_or_else(|err| panic!("{err}"))
    }

    pub fn try_parse_scene(self) -> Result<Scene, String> {
        let mut parser = Parser::new(self.src);
        if needs_var_prefetch(self.src) {
            parser.vars = Parser::new(self.src).try_collect_vars()?;
        }
        parser.parse_scene_inner()
    }

    pub(crate) fn try_parse_scene_lenient(self) -> ParseResult<Scene> {
        let mut parser = Parser::new_lenient(self.src);
        if needs_var_prefetch(self.src) {
            parser.vars = Parser::new_lenient(self.src).try_collect_vars()?;
        }
        parser.parse_scene_inner()
    }

    /// Parses a scene document, returning an error for invalid input.
    pub fn try_parse_scene_doc(self) -> Result<crate::SceneDoc, String> {
        crate::SceneDoc::try_parse_lenient(self.src)
    }

    /// Parses a scene document, panicking if the input is invalid.
    pub fn parse_scene_doc_or_panic(self) -> crate::SceneDoc {
        crate::SceneDoc::parse_lenient(self.src)
    }

    /// Parses a scene document, panicking if the input is invalid.
    ///
    /// Use [`Parser::try_parse_scene_doc`] for user-provided input.
    pub fn parse_scene_doc(self) -> crate::SceneDoc {
        self.parse_scene_doc_or_panic()
    }

    /// Parses one value literal, returning an error for invalid input.
    pub fn try_parse_value_literal(mut self) -> Result<SceneValue, String> {
        let value = self.parse_value()?;
        if self.current != Token::Eof {
            return Err(format!("Expected end of value, got {:?}", self.current));
        }
        Ok(value)
    }

    /// Parses one value literal, panicking if the input is invalid.
    pub fn parse_value_literal_or_panic(self) -> SceneValue {
        self.try_parse_value_literal()
            .unwrap_or_else(|err| panic!("{err}"))
    }

    /// Parses one value literal, panicking if the input is invalid.
    ///
    /// Use [`Parser::try_parse_value_literal`] for user-provided input.
    pub fn parse_value_literal(self) -> SceneValue {
        self.parse_value_literal_or_panic()
    }
}

fn ensure_value_fits_depth(value: &SceneValue, start_depth: usize) -> ParseResult<()> {
    let mut pending = vec![(value, start_depth)];
    while let Some((value, depth)) = pending.pop() {
        if depth > MAX_SCENE_VALUE_DEPTH {
            return Err(format!(
                "Scene value nesting exceeds limit of {MAX_SCENE_VALUE_DEPTH}"
            ));
        }
        match value {
            SceneValue::Object(fields) => {
                pending.extend(fields.iter().map(|(_, value)| (value, depth + 1)));
            }
            SceneValue::Array(items) => {
                pending.extend(items.iter().map(|value| (value, depth + 1)));
            }
            _ => {}
        }
    }
    Ok(())
}

fn custom_script_var_fields(fields: Vec<SceneObjectField>) -> Vec<SceneObjectField> {
    fields
        .into_iter()
        .map(|(name, value)| {
            (
                SceneFieldName::Custom(Cow::Owned(name.as_ref().to_string())),
                custom_script_var_value(value),
            )
        })
        .collect()
}

fn custom_script_var_value(value: SceneValue) -> SceneValue {
    match value {
        SceneValue::Object(fields) => {
            SceneValue::Object(Cow::Owned(custom_script_var_fields(fields.into_owned())))
        }
        SceneValue::Array(items) => SceneValue::Array(Cow::Owned(
            items
                .into_owned()
                .into_iter()
                .map(custom_script_var_value)
                .collect(),
        )),
        value => value,
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

fn canonical_node_type(name: &str) -> Option<NodeType> {
    match name {
        "UiScroll" => Some(NodeType::UiScrollContainer),
        "UiHBox" => Some(NodeType::UiHLayout),
        "UiVBox" => Some(NodeType::UiVLayout),
        "UiDropDown" => Some(NodeType::UiDropdown),
        other => NodeType::from_str(other).ok(),
    }
}

fn scene_node_data_from_parts(
    ty: &str,
    mut fields: Cow<'static, [SceneObjectField]>,
    base: Option<SceneNodeDataBase>,
) -> ParseResult<SceneNodeData> {
    if let Some(node_type) = canonical_node_type(ty) {
        normalize_node_fields_from_spec(node_type, fields.to_mut());
        Ok(SceneNodeData::new(node_type, fields, base))
    } else {
        Err(format!("unsupported scene node type `{ty}`"))
    }
}

fn canonical_scene_field_name(name: &str) -> SceneFieldName {
    SceneFieldName::from_borrowed(name).unwrap_or_else(|| SceneFieldName::from(name.to_string()))
}

fn normalize_node_fields_from_spec(node_type: NodeType, fields: &mut [SceneObjectField]) {
    let spec = scene_node_spec(node_type);
    for (name, value) in fields.iter_mut() {
        let Some(field) = spec.field(name.as_ref()) else {
            continue;
        };
        if name.as_ref() != field.name {
            *name = SceneFieldName::from(field.name);
        }
        if matches!(field.ty, NodeFieldType::Quat)
            && let SceneValue::Vec3 { x, y, z } = value.clone()
        {
            *value = euler_xyz_radians_to_quat_value(x, y, z);
        }
    }
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
    use super::{MAX_SCENE_VALUE_DEPTH, Parser};
    use crate::SceneValue;

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

    #[test]
    fn try_parse_scene_returns_parse_error() {
        let err = match Parser::new("$root = main\n\n[main]\n").try_parse_scene() {
            Ok(_) => panic!("expected parse error"),
            Err(err) => err,
        };
        assert!(err.contains("Expected"));
    }

    #[test]
    fn try_parse_scene_rejects_unterminated_type_block() {
        let err = Parser::new("$root = @main\n[main]\n[Node]\n")
            .try_parse_scene()
            .expect_err("invalid test input must fail");

        assert!(
            err.contains("Unterminated node type block `[Node]`"),
            "{err}"
        );
    }

    #[test]
    fn try_parse_scene_doc_returns_parse_error() {
        let err = Parser::new("$speed =")
            .try_parse_scene_doc()
            .expect_err("invalid test input must fail");
        assert!(err.contains("Invalid value token"), "{err}");
    }

    #[test]
    fn try_parse_value_literal_returns_parse_error() {
        let err = Parser::new("[1, 2")
            .try_parse_value_literal()
            .expect_err("invalid test input must fail");
        assert!(err.contains("Expected"), "{err}");
    }

    #[test]
    fn try_parse_value_literal_rejects_trailing_input() {
        let err = Parser::new("1 2")
            .try_parse_value_literal()
            .expect_err("invalid test input must fail");
        assert!(err.contains("Expected end of value"), "{err}");
    }

    #[test]
    fn parser_accepts_matrix_array_literal() {
        let value = Parser::new("[[1, 2, 3], [4, 5, 6]]").parse_value_literal();
        let SceneValue::Array(rows) = value else {
            panic!("expected matrix rows");
        };
        assert_eq!(rows.len(), 2);
        for row in rows.iter() {
            let SceneValue::Array(cols) = row else {
                panic!("expected matrix cols");
            };
            assert_eq!(cols.len(), 3);
        }
    }

    #[test]
    fn parser_rejects_value_over_depth_limit() {
        let src = format!(
            "{}0{}",
            "[".repeat(MAX_SCENE_VALUE_DEPTH + 1),
            "]".repeat(MAX_SCENE_VALUE_DEPTH + 1)
        );
        let err = Parser::new(&src)
            .try_parse_value_literal()
            .expect_err("invalid test input must fail");
        assert!(err.contains("nesting exceeds limit"), "{err}");
    }

    #[test]
    fn parser_rejects_variable_expansion_over_depth_limit() {
        let mut src = String::from("$v0 = 0\n");
        for depth in 1..=MAX_SCENE_VALUE_DEPTH + 1 {
            src.push_str(&format!("$v{depth} = [$v{}]\n", depth - 1));
        }
        let err = Parser::new(&src)
            .try_parse_scene()
            .expect_err("invalid test input must fail");
        assert!(err.contains("nesting exceeds limit"), "{err}");
    }
}
