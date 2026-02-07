use std::collections::HashMap;

use crate::{Lexer, Scene, SceneKey, SceneNode, SceneNodeData, SceneValue, Token};

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
}

impl<'a> Parser<'a> {
    fn collect_vars(mut self) -> HashMap<String, SceneValue> {
        while self.current != Token::Eof {
            if self.current == Token::At {
                self.advance();
                let name = self.expect_ident();
                if self.current == Token::Equals {
                    self.advance();
                    if name == "root" {
                        match &self.current {
                            Token::Ident(key) => {
                                let k = key.clone();
                                self.advance();
                                let scene_key = SceneKey(k);
                                self.vars
                                    .insert("root".to_string(), SceneValue::SceneKey(scene_key));
                            }
                            _ => panic!("root must be a scene key"),
                        }
                    } else {
                        let value = self.parse_value();
                        self.vars.insert(name, value);
                    }
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
                SceneValue::Str(v)
            }

            Token::At => {
                self.advance();
                let name = self.expect_ident();
                self.vars
                    .get(&name)
                    .cloned()
                    .unwrap_or_else(|| panic!("Unknown variable @{name}"))
            }

            // ✅ THIS IS THE MISSING PIECE
            Token::Ident(name) => {
                let key = name.clone();
                self.advance();
                SceneValue::SceneKey(SceneKey(key))
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

            _ => panic!("Invalid value token {:?}", self.current),
        }
    }
}

impl<'a> Parser<'a> {
    fn parse_type_block_after_lbracket(&mut self) -> SceneNodeData {
        let ty = self.expect_ident();
        self.expect(Token::RBracket);

        let mut fields = HashMap::new();
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
                    fields.insert(key, val);
                }

                _ => self.advance(),
            }
        }

        SceneNodeData { ty, fields, base }
    }

    fn parse_type_block(&mut self) -> SceneNodeData {
        self.expect(Token::LBracket);
        self.parse_type_block_after_lbracket()
    }
}

impl<'a> Parser<'a> {
    fn parse_scene_inner(mut self) -> Scene {
        let mut scene = Scene::default();

        while self.current != Token::Eof {
            match self.current {
                Token::At => {
                    self.advance();
                    let name = self.expect_ident();
                    self.expect(Token::Equals);

                    if name == "root" {
                        // root value must be a scene key (bare identifier)
                        match &self.current {
                            Token::Ident(key) => {
                                let k = key.clone();
                                self.advance();
                                let scene_key = SceneKey(k);
                                scene.root = Some(scene_key.clone());
                                // allow @root references later in the file
                                self.vars
                                    .insert("root".to_string(), SceneValue::SceneKey(scene_key));
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
                                    SceneValue::Str(s) => s,
                                    _ => panic!(),
                                })
                            }
                            "parent" => {
                                parent = Some(match v {
                                    SceneValue::SceneKey(k) => k,
                                    _ => panic!(),
                                })
                            }
                            "script" => {
                                script = Some(match v {
                                    SceneValue::Str(s) => s,
                                    _ => panic!(),
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

                    scene.nodes.insert(
                        SceneKey(key),
                        SceneNode {
                            name,
                            parent,
                            script,
                            data,
                        },
                    );
                }

                _ => self.advance(),
            }
        }

        scene.vars = self.vars.clone();
        scene
    }

    pub fn parse_scene(self) -> Scene {
        let vars = Parser::new(self.src).collect_vars();
        let mut parser = Parser::new(self.src);
        parser.vars = vars;
        parser.parse_scene_inner()
    }
}
