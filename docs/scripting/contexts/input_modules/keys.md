# Keys Module

Access:
- `ipt.Keys()`

Macros:
- `key_down!(ipt, key) -> bool`
- `key_pressed!(ipt, key) -> bool`
- `key_released!(ipt, key) -> bool`

Methods:
- `ipt.Keys().down(key) -> bool`
- `ipt.Keys().pressed(key) -> bool`
- `ipt.Keys().released(key) -> bool`

Inputs:
- `key: KeyCode`

Available `KeyCode` values are defined by the engine enum and include:

- Punctuation and symbols:
  - `Backquote`, `Backslash`, `BracketLeft`, `BracketRight`, `Comma`, `Equal`, `Minus`, `Period`, `Quote`, `Semicolon`, `Slash`
- Number row:
  - `Digit0`..`Digit9`
- Letter keys:
  - `KeyA`..`KeyZ`
- Modifiers and editing:
  - `AltLeft`, `AltRight`, `Backspace`, `CapsLock`, `ContextMenu`, `ControlLeft`, `ControlRight`, `Enter`, `SuperLeft`, `SuperRight`, `ShiftLeft`, `ShiftRight`, `Space`, `Tab`
- International / IME:
  - `IntlBackslash`, `IntlRo`, `IntlYen`, `Convert`, `KanaMode`, `Lang1`, `Lang2`, `Lang3`, `Lang4`, `Lang5`, `NonConvert`, `Hiragana`, `Katakana`
- Navigation:
  - `Delete`, `End`, `Help`, `Home`, `Insert`, `PageDown`, `PageUp`, `ArrowDown`, `ArrowLeft`, `ArrowRight`, `ArrowUp`
- Numpad:
  - `NumLock`, `Numpad0`..`Numpad9`, `NumpadAdd`, `NumpadBackspace`, `NumpadClear`, `NumpadClearEntry`, `NumpadComma`, `NumpadDecimal`, `NumpadDivide`, `NumpadEnter`, `NumpadEqual`, `NumpadHash`, `NumpadMemoryAdd`, `NumpadMemoryClear`, `NumpadMemoryRecall`, `NumpadMemoryStore`, `NumpadMemorySubtract`, `NumpadMultiply`, `NumpadParenLeft`, `NumpadParenRight`, `NumpadStar`, `NumpadSubtract`
- System and browser/media:
  - `Escape`, `Fn`, `FnLock`, `PrintScreen`, `ScrollLock`, `Pause`, `BrowserBack`, `BrowserFavorites`, `BrowserForward`, `BrowserHome`, `BrowserRefresh`, `BrowserSearch`, `BrowserStop`, `Eject`, `LaunchApp1`, `LaunchApp2`, `LaunchMail`, `MediaPlayPause`, `MediaSelect`, `MediaStop`, `MediaTrackNext`, `MediaTrackPrevious`, `Power`, `Sleep`, `AudioVolumeDown`, `AudioVolumeMute`, `AudioVolumeUp`, `WakeUp`
- Extended/control:
  - `Meta`, `Hyper`, `Turbo`, `Abort`, `Resume`, `Suspend`, `Again`, `Copy`, `Cut`, `Find`, `Open`, `Paste`, `Props`, `Select`, `Undo`
- Function keys:
  - `F1`..`F35`

Source of truth:
- `perro_source/api_modules/perro_input/src/keycode.rs`
