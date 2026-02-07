// parser.rs - Parse into runtime types
use crate::{Lexer, RuntimeNodeData, RuntimeNodeEntry, RuntimeScene, RuntimeValue, Token};
use std::collections::HashMap;

pub struct Parser<'a> {
    src: &'a str,
    lexer: Lexer<'a>,
    current: Token,
    vars: HashMap<String, RuntimeValue>,
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

    fn collect_vars(mut self) -> HashMap<String, RuntimeValue> {
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

    fn parse_value(&mut self) -> RuntimeValue {
        match &self.current {
            Token::Number(n) => {
                let v = *n;
                self.advance();
                RuntimeValue::F32(v)
            }

            Token::String(s) => {
                let v = s.clone();
                self.advance();
                RuntimeValue::Str(v)
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
                RuntimeValue::Key(key)
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
                    2 => RuntimeValue::Vec2 {
                        x: nums[0],
                        y: nums[1],
                    },
                    3 => RuntimeValue::Vec3 {
                        x: nums[0],
                        y: nums[1],
                        z: nums[2],
                    },
                    4 => RuntimeValue::Vec4 {
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
                RuntimeValue::Bool(true)
            }

            Token::False => {
                self.advance();
                RuntimeValue::Bool(false)
            }

            _ => panic!("Invalid value token {:?}", self.current),
        }
    }

    fn parse_type_block_after_lbracket(&mut self) -> RuntimeNodeData {
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
                        base = Some(Box::new(nested));
                    }
                }

                Token::Ident(_) => {
                    let key = self.expect_ident();
                    self.expect(Token::Equals);
                    let val = self.parse_value();
                    fields.push((key, val));
                }

                _ => self.advance(),
            }
        }

        RuntimeNodeData { ty, fields, base }
    }

    fn parse_type_block(&mut self) -> RuntimeNodeData {
        self.expect(Token::LBracket);
        self.parse_type_block_after_lbracket()
    }

    fn parse_scene_inner(mut self) -> RuntimeScene {
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
                                root = Some(k.clone());
                                self.vars.insert("root".to_string(), RuntimeValue::Key(k));
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
                    let mut parent = None;
                    let mut script = None;

                    while matches!(self.current, Token::Ident(_)) {
                        let k = self.expect_ident();
                        self.expect(Token::Equals);
                        let v = self.parse_value();
                        match k.as_ref() {
                            "name" => {
                                name = Some(match v {
                                    RuntimeValue::Str(s) => s,
                                    _ => panic!("name must be a string"),
                                })
                            }
                            "parent" => {
                                parent = Some(match v {
                                    RuntimeValue::Key(k) => k,
                                    _ => panic!("parent must be a key"),
                                })
                            }
                            "script" => {
                                script = Some(match v {
                                    RuntimeValue::Str(s) => s,
                                    _ => panic!("script must be a string"),
                                })
                            }
                            _ => {}
                        }
                    }

                    let data = self.parse_type_block();

                    self.expect(Token::LBracket);
                    self.expect(Token::Slash);
                    let end = self.expect_ident();
                    self.expect(Token::RBracket);
                    assert_eq!(end, key);

                    nodes.push(RuntimeNodeEntry {
                        key,
                        name,
                        parent,
                        script,
                        data,
                    });
                }

                _ => self.advance(),
            }
        }

        RuntimeScene { nodes, root }
    }

    pub fn parse_scene(self) -> RuntimeScene {
        let vars = Parser::new(self.src).collect_vars();
        let mut parser = Parser::new(self.src);
        parser.vars = vars;
        parser.parse_scene_inner()
    }
}
