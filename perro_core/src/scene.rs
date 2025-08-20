use crate::{api::{self, ScriptApi}, ast::{FurElement, FurNode}, get_project_root, nodes::scene_node::SceneNode, parse_fur::{build_ui_elements_from_fur, parse_fur_file}, resolve_res_path, scene_node::BaseNode, script::{CreateFn, Script, UpdateOp, Var}, scripting, ui_element::{BaseElement, UIElement}, ui_renderer::render_ui, Graphics, Sprite2D, Vector2};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::{
    any::Any, cell::RefCell, collections::HashMap, fs::{create_dir_all, File}, hash::Hash, io::{self, BufReader}, mem, path::PathBuf, rc::Rc
};
use uuid::Uuid;
use wgpu::RenderPass;
use libloading::{Library, Symbol};
use anyhow::Result;


fn transpiled_path(original_path: &str) -> String {
    let name = original_path
        .rsplit('/')
        .next()
        .unwrap_or("unknown.pup")
        .trim_end_matches(".pup");
    format!("res://transpiled/{}.rs", name)
}


#[derive(Serialize, Deserialize)]
pub struct Scene {
    pub root_id: Uuid,
    pub nodes:   IndexMap<Uuid, SceneNode>,

    #[serde(skip, default)]
    perro_rust_lib: Option<Library>,          // only this one is optional

    #[serde(skip, default)]
    constructors: HashMap<String, CreateFn>,  // plain HashMap

    #[serde(skip, default)]
    pub scripts: HashMap<Uuid, Rc<RefCell<Box<dyn Script>>>>,

    pub is_game_scene: bool,
}

fn default_perro_rust_path() -> std::path::PathBuf {

    let project_root = get_project_root();
    // Which profile are we using?
    let profile = if cfg!(debug_assertions) { "hotreload" } else { "release" };

    // Start from the project root
    let mut path = project_root;

    // Go into the transpiled crate target dir
    path.push(".perro");
    path.push("rust_scripts");
    path.push("target");
    path.push(profile);

    // OS-specific library name
    let filename = if cfg!(target_os = "windows") {
        "scripts.dll"
    } else if cfg!(target_os = "macos") {
        "libscripts.dylib"
    } else {
        "libscripts.so"
    };

    path.push(filename);
    path
}

impl Scene {
    /// Constructor for the game-runtime view (loads the DLL, flag = true)
    pub fn new(root: SceneNode, is_game_scene: bool) -> anyhow::Result<Self> {
        let root_id = *root.get_id();
        let mut nodes = IndexMap::new();
        nodes.insert(root_id, root);

        let perro_rust_lib = if is_game_scene {
            let lib_path = default_perro_rust_path();
            println!("Loading scripts from {}", lib_path.display());
            Some(unsafe { Library::new(&lib_path)? })
        } else {
            None
        };

        Ok(Self {
            root_id,
            nodes,
            perro_rust_lib,
            constructors: HashMap::new(),
            scripts: HashMap::new(),
            is_game_scene,
        })
    }

   fn ctor(&mut self, short: &str) -> anyhow::Result<CreateFn> {
    if let Some(&f) = self.constructors.get(short) {
        println!("ctor cache hit: {short}");
        return Ok(f);
    }

    let lib = self.perro_rust_lib.as_ref().unwrap();
    let symbol = format!("{short}_create_script\0");
    println!("loading symbol `{symbol}`");
    let sym: libloading::Symbol<CreateFn> = unsafe { lib.get(symbol.as_bytes())? };
    println!("sym loaded");
    let fptr = *sym;
    self.constructors.insert(short.to_owned(), fptr);
    Ok(fptr)
}

    pub fn tick(
        &mut self,
        gfx: &mut Graphics,
        pass: &mut wgpu::RenderPass<'_>,
        delta: f32,
    ) {
        self.process(delta);          // game-logic, scripts
        self.render(gfx, pass);  // draw
    }
    
 pub fn process(&mut self, delta: f32) {
        // 1) Snapshot keys so we don't mutate the map in the loop
        let ids: Vec<Uuid> = self.scripts.keys().cloned().collect();

        for id in ids {
            // 2) Create your API (which *does* borrow &mut self)
            let mut api = ScriptApi::new(delta, self);
            // 3) Call a method on the API that looks up & updates one script
            api.call_update(id);
        }
    }
    
    pub fn save(&self, res_path: &str) -> io::Result<()> {
        let path = resolve_res_path(res_path);
        if let Some(dir) = path.parent() {
            create_dir_all(dir)?;
        }
        let file = File::create(&path)?;
        serde_json::to_writer_pretty(file, &self)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
    }

   pub fn load(res_path: &str) -> io::Result<Self> {
        // 0) deserialize the raw scene (root_id + nodes map)
        let mut scene: Scene = {
            let path = resolve_res_path(res_path);
            let file = File::open(&path)?;
            let reader = BufReader::new(file);
            serde_json::from_reader(reader)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?
        };

        // 1) Fix each node's internal `id` field from its HashMap key:
        for (&key, node) in scene.nodes.iter_mut() {
            node.set_id(key);
        }

        // 2) Clear all existing children lists (we'll rebuild them):
        for node in scene.nodes.values_mut() {
            node.get_children_mut().clear();
        }

        // 3) Collect child→parent pairs and rebuild each parent's list:
        let child_parent_pairs: Vec<(Uuid, Uuid)> = scene
            .nodes
            .iter()
            .filter_map(|(&child_id, node)| {
                node.get_parent().map(|pid| (child_id, pid))
            })
            .collect();

        for (child_id, parent_id) in child_parent_pairs {
            if let Some(parent) = scene.nodes.get_mut(&parent_id) {
                parent.add_child(child_id);
            }
        }

        Ok(scene)
    }

 pub fn graft(
        &mut self,
        other: Scene,
        attach_to: Uuid,
    ) -> anyhow::Result<()> {
        let other_root = other.root_id;

        for (id, node) in other.nodes.into_iter() {
            // run your normal script‐loading / node‐insertion path:
            self.create_node(node)?;

            // if this was the root of the grafted scene, reparent it:
            if id == other_root {
                // 1) set its flat parent pointer
                let root_node = self
                    .nodes
                    .get_mut(&id)
                    .expect("just inserted");
                root_node.set_parent(Some(attach_to));

                // 2) append it to the attach_to's children
                let parent_node = self
                    .nodes
                    .get_mut(&attach_to)
                    .expect("attach_to existed already");
                parent_node.add_child(id);
            }
        }

        // preserve your existing scene‐root flag
        self.is_game_scene = false;
        Ok(())
    }

    
    pub fn get_root(&self) -> &SceneNode {
        &self.nodes[&self.root_id]
    }

    pub fn get_node<T: 'static>(&self, id: &Uuid) -> Option<&T> {
        self.nodes.get(id)
            .and_then(|node| node.as_any().downcast_ref::<T>())
    }


    pub fn get_node_mut<T: 'static>(&mut self, id: &Uuid) -> Option<&mut T> {
        self.nodes.get_mut(id)
            .and_then(|node| node.as_any_mut().downcast_mut::<T>())
    }


    pub fn update_script_var(
        &mut self,
        node_id: &Uuid,
        name: &str,
        op: UpdateOp,
        val: Var,
    ) -> Option<()> {

        let rc_script = self.scripts.get(node_id)?;
        
        let mut script = rc_script.borrow_mut();

        let current = script.get_var(name)?;  // get current value
        

        let new_val = match op {
            UpdateOp::Set => val,
            UpdateOp::Add => current + val,
            UpdateOp::Sub => current - val,
            UpdateOp::Mul => current * val,
            UpdateOp::Div => current / val,
            UpdateOp::Rem => current % val,
            UpdateOp::And => current & val,
            UpdateOp::Or  => current | val,
            UpdateOp::Xor => current ^ val,
            UpdateOp::Shl => current << val,
            UpdateOp::Shr => current >> val,
        };

        script.set_var(name, new_val)?;
        Some(())
    }


  pub fn create_node(&mut self, mut node: SceneNode) -> anyhow::Result<()> {
    let id = *node.get_id();

       if let SceneNode::UI(ref mut ui_node) = node {
        if let Some(fur_path) = &ui_node.fur_path {
        

            match parse_fur_file(fur_path) {
                Ok(ast) => {
                  

                    // Extract FurElements from FurNodes at root level
                    let fur_elements: Vec<FurElement> = ast.into_iter()
                        .filter_map(|fur_node| {
                            if let FurNode::Element(el) = fur_node {
                                Some(el)
                            } else {
                                None
                            }
                        })
                        .collect();

                    // Build UI elements and insert directly into ui_node.elements
                    build_ui_elements_from_fur(ui_node, &fur_elements);

    
                }
                Err(err) => {
                    println!("Error parsing .fur file: {}", err);
                }
            }
        }
    }

    if let Some(pup_path) = node.get_script_path().cloned() {
        let short = std::path::Path::new(&pup_path)
            .file_stem()
            .unwrap()
            .to_string_lossy();
        println!("  wants script `{short}`");



        let ctor = self.ctor(&short)?;          // may fail → bail early
        println!("  constructor loaded at {:p}", ctor as *const ());

        let raw = unsafe { ctor() };

        // 2) turn it into a Box<dyn Script>
        let mut boxed: Box<dyn Script> = unsafe { Box::from_raw(raw) };
        boxed.set_node_id(id);

        // 3) wrap that Box in RefCell + Rc
        let handle: Rc<RefCell<Box<dyn Script>>> = Rc::new(RefCell::new(boxed));

        // 4) init it
        {
            let mut api = ScriptApi::new(0.0, self);
            handle.borrow_mut().init(&mut api);
        }

        // 5) store it
        self.scripts.insert(id, handle);
        println!("  script instance stored");
    }

    self.nodes.insert(id, node);
    Ok(())
}

    pub fn remove_node(&mut self, id: &Uuid) -> Option<SceneNode> {
        if let Some(node) = self.nodes.remove(id) {
            if let Some(pid) = node.get_parent() {
                if let Some(parent) = self.nodes.get_mut(&pid) {
                    parent.remove_child(id);
                }
            }
            Some(node)
        } else {
            None
        }
    }

    pub fn add_child(&mut self, parent_id: Uuid, child_id: Uuid) {
        if let Some(child) = self.nodes.get_mut(&child_id) {
            child.set_parent(Some(parent_id));
        }
        if let Some(parent) = self.nodes.get_mut(&parent_id) {
            parent.add_child(child_id);
        }
    }

    pub fn remove_child(&mut self, parent_id: Uuid, child_id: Uuid) {
        if let Some(parent) = self.nodes.get_mut(&parent_id) {
            parent.remove_child(&child_id);
        }
        if let Some(child) = self.nodes.get_mut(&child_id) {
            if child.get_parent() == Some(parent_id) {
                child.set_parent(None);
            }
        }
    }

    pub fn set_parent(&mut self, node_id: Uuid, new_parent: Option<Uuid>) {
        let old_parent = {
            let node = self.nodes.get_mut(&node_id).unwrap();
            let old = node.get_parent();
            node.set_parent(new_parent);
            old
        };
        if let Some(op) = old_parent {
            if let Some(parent) = self.nodes.get_mut(&op) {
                parent.remove_child(&node_id);
            }
        }
        if let Some(np) = new_parent {
            if let Some(parent) = self.nodes.get_mut(&np) {
                parent.add_child(node_id);
            }
        }
    }



    pub fn traverse<F>(&self, start: Uuid, visit: &mut F)
        where
            F: FnMut(&SceneNode),
        {
            // If the ID is missing we just stop; no unwrap / panic.
            if let Some(node) = self.nodes.get(&start) {
                visit(node);
                for &child in node.get_children() {
                    self.traverse(child, visit);
                }
            }
        }

        pub fn render(&self, gfx: &mut Graphics, pass: &mut RenderPass<'_>) {
            // Early-out if the root has somehow been deleted
            if !self.nodes.contains_key(&self.root_id) {
                return;
            }

            self.traverse(self.root_id, &mut |node| {
                match node {
                    SceneNode::Sprite2D(sprite) if sprite.visible => {
                        if let Some(tex) = &sprite.texture_path {
                            gfx.draw_image_in_pass(pass, tex, sprite.transform.clone(), Vector2::new(0.5, 0.5));
                        }
                    }
                    SceneNode::UI(ui_node) => { render_ui(ui_node, gfx, pass);},
                    _ => {} // Node2D or Node: nothing to draw
                }
            });
        }
}
