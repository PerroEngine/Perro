# Script Teamwork Feature Stories

Each story starts with owners and data flow, then explains why an API fits and
what fails safely. Use the smallest communication path that matches what the
caller knows. A feature does not need every path.

- [Player, Camera, And HUD](player_camera_hud.md) -> fixed ref + typed node + signal
- [Switch Calls Door](call_method.md) -> one target + return value
- [Manager And Spawned Enemies](spawned_enemies.md) -> dynamic membership
- [Pickup, Inventory, And UI](pickup_flow.md) -> method + signal + adapter
- [Scene-Injected Asset Variants](asset_variants.md) -> path -> typed nested state
- [Timer-Driven Cooldown](cooldown.md) -> delayed state transition
- [Dynamic Inspector Adapter](dynamic_vars.md) -> runtime-selected member
- [Player Health Signal Updates HUD](signals.md) -> focused signal example

Runnable source: [ScriptPatterns](https://github.com/PerroEngine/Perro/tree/main/demos/ScriptPatterns).

[Back To Authoring Guide](../index.md)
