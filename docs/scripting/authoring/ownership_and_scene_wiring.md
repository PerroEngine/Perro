# Ownership And Scene Wiring

## Purpose

Place data and behavior with the node responsible for them. Let the scene wire
dependencies that are already known while authoring the scene.

## Mental Model

```text
scene owns topology + instance choices
node owns concrete engine data
script state owns gameplay data for that node
method owns targeted behavior
signal describes a fact without owning listeners
```

## Ownership Rules

- Put health on the player script, not the HUD.
- Put open/close behavior on the door, not the switch.
- Put a fixed door `NodeID` on the switch because the switch needs the door.
- Put a per-instance portrait `TextureID` on the character state.
- Put scene-wide wave order on a manager, while each enemy owns movement and HP.

Scene wiring makes dependencies reviewable:

```text
[Switch]
script = "res://scripts/switch.rs"
script_vars = { door = @ExitDoor }
```

`#[expose]` only organizes fields in the editor inspector. Every state field can
receive a scene value. Use it for authoring clarity, not as an access or
injection gate.

## When Not To Inject

Do not inject constants shared by every instance; use Rust constants. Do not
inject a temporary callback result; use a local. Do not inject a dynamic set of
spawned enemies; query the tag when the set is needed or maintain a registry.

## AI Authoring Contract

Every generated example must make these facts explicit:

- owner: node/script responsible for value or behavior
- source: scene, state, node, event params, or runtime query
- target: `ctx.id`, injected ref, relation, or query result
- lifecycle: callback that starts the flow
- failure: missing target, decode failure, or neutral return
- access: typed path unless name/type is truly dynamic

## Related

- [References And Queries](references_and_queries.md)
- [Script Communication](communication.md)
- [State And References](state_and_refs.md)

