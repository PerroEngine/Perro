# UI, Animation, Audio

Use UI, animation, and audio for feedback and game feel.

## Goal

Add visible state, motion, and sound.

## UI

Use UI nodes for menus and HUD.

Common nodes:

- labels
- buttons
- images
- layout nodes
- scroll containers
- text input

Use UI styles for shared visual states.

## Animation

Use animation resources for authored motion.

Use animation trees for state-based playback.

Use script state for game decisions.

Use animation resources for timed values.

## Audio

Use `ctx.res.Audio()` for resource-level playback.

Use runtime/node audio APIs for attached spatial playback.

Use buses and zones for mix control.

Use MIDI and soundfonts when procedural notes or compact music data fit.

## Feedback Rule

Every player action should map to at least one feedback layer:

- visual state
- animation
- sound
- UI pulse
- particle or light response

## Reference

- [UI Nodes](/docs/scripting/ui.md)
- [`.uistyle` Format](/docs/resources/uistyle.md)
- [Animation](/docs/resources/animation.md)
- [`.panim` Format](/docs/resources/panim.md)
- [`.panimtree` Format](/docs/resources/panimtree.md)
- [Audio](/docs/resources/audio.md)
- [Audio Module](/docs/scripting/contexts/resource_modules/audio.md)
- [Runtime Audio Module](/docs/scripting/contexts/runtime_modules/audio.md)
