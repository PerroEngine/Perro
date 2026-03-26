# Math Types

Perro scripting uses `Vector2`, `Vector3`, and `Quaternion` from `perro_structs` for common math operations.

## Vector2

```rust
let a = Vector2::new(0.0, 0.0);
let b = Vector2::new(3.0, 4.0);

let d = a.distance_to(b);          // 5.0
let dir = a.direction_to(b);       // (0.6, 0.8)
let ang = Vector2::new(1.0, 0.0)
    .angle_to(Vector2::new(0.0, 1.0)); // PI/2
```

Methods:
- `distance_to(other)`
- `direction_to(other)`
- `angle_to(other)`
- `dot(rhs)`, `cross(rhs)`
- `length()`, `length_squared()`, `normalized()`
- `lerped(to, t)`
- `lerp(to, t)`

## Vector3

```rust
let from = Vector3::new(0.0, 0.0, 0.0);
let to = Vector3::new(0.0, 0.0, -10.0);

let d = from.distance_to(to);      // 10.0
let dir = from.direction_to(to);   // (0, 0, -1)

let v = Vector3::new(2.0, 3.0, 0.0);
let x_axis = Vector3::new(1.0, 0.0, 0.0);
let on_x = v.project_on(x_axis);   // (2, 0, 0)
```

Methods:
- `distance_to(other)`
- `direction_to(other)`
- `angle_to(other)`
- `project_on(onto)`
- `dot(rhs)`, `cross(rhs)`
- `length()`, `length_squared()`, `normalized()`
- `lerped(to, t)`
- `lerp(to, t)`

## Quaternion

Use quaternions for stable 3D rotation and aiming.

```rust
let q = Quaternion::looking_at(
    Vector3::new(0.0, 0.0, -1.0),
    Vector3::new(0.0, 1.0, 0.0),
);

let forward = q.rotate_vector3(Vector3::new(0.0, 0.0, -1.0));

let blended = Quaternion::IDENTITY.slerped(q, 0.5);
```

Methods:
- `looking_at(direction, up)`
- `look_at(direction, up)`
- `rotate_vector3(v)`
- `slerped(to, t)`
- `slerp(to, t)`
- `inverse()`, `normalized()`, `normalize()`
- `mul_quat(rhs)`
- `rotate_x(radians)`, `rotate_y(radians)`, `rotate_z(radians)`, `rotate_xyz(x,y,z)`

## Structs

Primary math structs in scripting:
- `Vector2`
- `Vector3`
- `Quaternion`
- `Transform2D`
- `Transform3D`
