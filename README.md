# Perro Game Engine

<div align="center">
  <img src="icon.png" alt="Perro Logo" width="200"/>
</div>

**Perro** is an experimental, open-source game engine written in **Rust**, built as a modern alternative to engines like Unreal, Godot, and Unity. It focuses on **simplicity** of making games without sacrificing **performance**.

## Rust as a Scripting Language

While **Rust** is typically a low-level systems language, Perro uses it as a scripting language for games programming. Behavior scripts are organized into clear sections: `#[State]` data, lifecycle entry points (`lifecycle!`), and behavior methods (`methods!`). Perro also supports bare Rust modules in `res/**.rs` for shared functions/constants/types.

This system is structured and architected as such to make scripts simple to write and clearly lay out access to runtime state (script/node mutations/reads)

For more details, see the full documentation: [perroengine.com/docs](https://www.perroengine.com/docs).

Local reference:

- [Docs Index](docs/index.md)
- [Perro CLI](docs/perro_cli.md)

## Contributions

Perro is, of course, **open source**, and contributions are always appreciated: issue reports, new features, system optimizations, and other improvements. Everyone is welcome to join the project.

## Support Perro

Donations help fund full-time development, faster features, and better tooling. If you want to support the project:

- [Support Directly](https://perroengine.com/sponsor)
- [Support on Ko-fi](https://ko-fi.com/perroengine)

---

## License

Perro is licensed under the **Apache 2.0 License**. See [LICENSE](LICENSE) for details.

---
