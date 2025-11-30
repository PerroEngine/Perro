// Basic TypeScript test script extending Node
// Type definitions for structs (TypeScript interfaces)
interface TestVector {
    x: number;
    y: number;
}

interface GameEntity {
    entity_id: number;
    entity_name: string;
    entity_type: string;
}

interface TestPlayer extends GameEntity {
    pos: TestVector;
    health: number;
    mana: number;
}

interface SuperTestPlayer extends TestPlayer {
    special_ability: string;
    energy_core: number;
}


class Types extends Node {
    // =====================================================
    // TOP-LEVEL EXPOSED & INTERNAL VARIABLES (DEPENDENCY ORDERED)
    // =====================================================

    // --- Primitives: Default, Specific Widths, BigInt, Decimal ---
    public untyped_num_default: number = 10;
    public typed_int_default: number = 20;
    public typed_int_8: number = -120;
    public typed_int_64: number = 1_000_000_000_000;
    public typed_uint_16: number = 60000;
    public typed_float_default: number = 30.5;
    public typed_float_64: number = 123.456_789;
    public typed_big_int: bigint = BigInt("12345678901234567890");
    public typed_decimal: number = 987.6543210987654321;
    public typed_string: string = "HelloPerro";

    // Local Primitives (defined before custom structs that might use them)
    public local_int: number = 5;
    private local_uint_8: number = 250;
    private local_float: number = 2.5;
    private local_string: string = "LocalString";
    private local_big_int: bigint = BigInt("1000000000000000000");
    private local_decimal: number = 12.34567890123456789;

    

    // --- Custom Struct Definitions (must precede their usage) ---
    public testVector: TestVector;
    public gameEntity: GameEntity;
    public testPlayer: TestPlayer;
    public superTestPlayer: SuperTestPlayer;

    // --- Custom Struct Instances (DEFINED IN ORDER OF DEPENDENCY) ---
    public my_base_entity: GameEntity = {
        entity_id: 100,
        entity_name: "NPC_Guard",
        entity_type: "NPC"
    };

    // CORRECTED: TestPlayer initializer is flat, setting all direct & inherited fields
    public my_player: TestPlayer = {
        entity_id: 1,      // Inherited from GameEntity
        entity_name: "Hero", // Inherited from GameEntity
        entity_type: "Player", // Inherited from GameEntity
        pos: { x: 0.0, y: 0.0 }, // Direct field
        health: 100.0,     // Direct field
        mana: 50           // Direct field
    };

    // Local Player Instance (defined before containers that use it)
    private other_player: TestPlayer = {
        entity_id: 2,
        entity_name: "Sidekick",
        entity_type: "Player",
        pos: { x: 5.0, y: 10.0 },
        health: 80.0,
        mana: 30
    };

    // CORRECTED: SuperTestPlayer initializer is flat, setting all direct & inherited fields
    public my_derived_player: SuperTestPlayer = {
        entity_id: 3,      // Inherited from GameEntity
        entity_name: "SuperHero", // Inherited from GameEntity
        entity_type: "SuperPlayer", // Inherited from GameEntity
        pos: { x: 10.0, y: 10.0 }, // Inherited from TestPlayer
        health: 200.0,     // Inherited from TestPlayer
        mana: 100,         // Inherited from TestPlayer
        special_ability: "Flight", // Direct field
        energy_core: 99.9  // Direct field
    };

    // Local SuperTestPlayer Instance (defined before containers that use it)
    private my_derived_player_var: SuperTestPlayer = {
        entity_id: 4,
        entity_name: "LocalHero",
        entity_type: "LocalPlayer",
        pos: { x: 20.0, y: 20.0 },
        health: 180.0,
        mana: 80,
        special_ability: "Speed",
        energy_core: 80.0
    };

    // --- Dynamic Containers (always Array<any> / Map<string, any> in TypeScript) ---
    public dynamic_array_inferred: any[] = [1, "two", 3.0, this.untyped_num_default, this.typed_big_int, this.typed_decimal];
    public dynamic_map_inferred: Map<string, any> = new Map<string, any>([
        ["alpha", 1],
        ["beta", this.typed_string],
        ["gamma", this.typed_big_int],
        ["delta", this.typed_decimal]
    ]);

    // Annotated containers now correctly reference defined variables
    public dynamic_array_annotated: any[] = [this.typed_int_default, this.typed_float_64, this.my_derived_player_var, this.typed_big_int];
    public dynamic_map_annotated: Map<string, any> = new Map<string, any>([
        ["char", "A"],
        ["num", this.typed_int_8],
        ["big_val", this.local_big_int]
    ]);

    // --- Statically Typed Arrays ---
    public static_array_int: number[] = [10, 20, 30];
    public static_array_uint_16: number[] = [1000, 2000];
    public static_array_string: string[] = ["one", "two", "three"];
    public static_array_float_64: number[] = [1.11, 2.22, 3.33];
    public static_array_big_int: bigint[] = [BigInt(100), BigInt(200), this.typed_big_int];
    public static_array_decimal: number[] = [5.5, 6.6, this.typed_decimal];

    // --- Statically Typed Maps ---
    public static_map_string_int_64: Map<string, number> = new Map([
        ["level", 10_000_000_000],
        ["score", 1_000_000_000]
    ]);
    public static_map_int_string: Map<number, string> = new Map([
        [1, "gold"],
        [2, "silver"]
    ]);
    public static_map_uint_8_float: Map<number, number> = new Map([
        [50, 0.5],
        [100, 1.5]
    ]);
    public static_map_string_big: Map<string, bigint> = new Map([
        ["large_num", BigInt("9999999999999999999")],
        ["another_large", BigInt(1000)]
    ]);
    public static_map_string_decimal: Map<string, number> = new Map([
        ["price", 19.99],
        ["tax", 1.50]
    ]);

    // --- Statically Typed Containers of Custom Structs (using correctly initialized instances) ---
    public static_array_entities: GameEntity[] = [this.my_base_entity, this.my_player, this.my_derived_player];
    public static_array_players: TestPlayer[] = [this.my_player, this.other_player];
    public static_map_players: Map<string, TestPlayer> = new Map([
        ["main", this.my_player],
        ["other", this.other_player]
    ]);
    public static_map_super_players: Map<string, SuperTestPlayer> = new Map([
        ["super", this.my_derived_player],
        ["local_super", this.my_derived_player_var]
    ]);

    // =====================================================
    // TEST SUITE FUNCTIONS
    // =====================================================

    // --- Test 1: Primitive Operations (literals, variables, promotion) ---
    public test_primitive_operations(): void {
        console.log("--- Test Primitive Operations ---");

        // Variable + Literal
        let res_int: number = this.typed_int_default + 10;
        let res_big: bigint = this.typed_big_int + BigInt(1000);
        let res_decimal: number = this.typed_decimal + 1.000001;
        console.log("Var + Lit:", res_int, res_big, res_decimal);

        // Variable + Variable
        let res_big_var: bigint = this.typed_big_int + this.local_big_int;
        let res_decimal_var: number = this.typed_decimal + this.local_decimal;
        console.log("Var + Var (Big/Dec):", res_big_var, res_decimal_var);

        // Type Promotion with BigInt/Decimal
        let prom_float_big: number = Number(this.typed_int_64) + Number(this.typed_big_int);
        let prom_float_decimal: number = this.typed_float_64 + this.typed_decimal;
        let prom_big_int: bigint = BigInt(this.typed_int_64) + this.typed_big_int;
        console.log("Promotion (Big/Dec):", prom_float_big, prom_float_decimal, prom_big_int);
    }

    // --- Test 2: Explicit Type Casting ---
    public test_explicit_casting(): void {
        console.log("--- Test Explicit Casting ---");

        // Primitive Variable to Primitive Variable (various widths, Big/Decimal)
        let int64_to_big: bigint = BigInt(this.typed_int_64);
        let big_to_int: number = Number(this.typed_big_int);
        let float_to_decimal: number = this.typed_float_default;
        let decimal_to_float_64: number = this.typed_decimal;
        let string_to_uint16: number = parseInt("65530", 10);
        let big_to_string: string = this.typed_big_int.toString();
        console.log("Numeric Casts:", int64_to_big, big_to_int, float_to_decimal, decimal_to_float_64, string_to_uint16, big_to_string);

        // Dynamic Value to Primitive (various widths, Big/Decimal)
        let dyn_val_big: bigint = this.dynamic_array_inferred[4] as bigint;
        let dyn_val_decimal: number = this.dynamic_array_inferred[5] as number;
        console.log("Dyn->Big/Dec Casts:", dyn_val_big, dyn_val_decimal);

        // Cast chain and operations
        let casted_and_op_big: bigint = (this.dynamic_array_inferred[0] as bigint) + this.typed_big_int;
        console.log("Casted & Op (Big):", casted_and_op_big);
    }

    // --- Test 3: Assignments (Simple & Compound) ---
    public test_assignments(): void {
        console.log("--- Test Assignments (Simple & Compound) ---");

        // Simple Assignment (=)
        let assign_big_lit: bigint = BigInt(999);
        let assign_decimal_var: number = this.typed_decimal;
        console.log("Simple Assign (Big/Dec):", assign_big_lit, assign_decimal_var);

        // Compound Assignment (+=, -=) with BigInt/Decimal
        let comp_big: bigint = BigInt(100);
        comp_big = comp_big + BigInt(50);
        console.log("Comp Assign big +=", comp_big);

        let comp_decimal: number = 20.0;
        comp_decimal = comp_decimal - 5.5;
        console.log("Comp Assign decimal -=", comp_decimal);

        // Assignments with Type Promotion/Casting
        let assign_prom_decimal: number = this.typed_int_default;
        assign_prom_decimal = assign_prom_decimal + this.typed_float_default;
        console.log("Assign Promo decimal:", assign_prom_decimal);

        // Member Assignment with BigInt/Decimal (for existing float fields with casts)
        this.my_player.pos.x = Number(this.typed_big_int);
        this.my_player.health = this.typed_decimal;
        console.log("Member Assign (Big/Dec to float):", this.my_player.pos.x, this.my_player.health);
    }

    // --- Test 4: Struct Inheritance, Member Access, Casting between Base/Derived ---
    public test_struct_inheritance_and_casting(): void {
        console.log("--- Test Struct Inheritance & Casting ---");

        // Direct Access to inherited fields (as TypeScript implies)
        console.log("Player name (via SuperTestPlayer):", this.my_derived_player.entity_name);
        console.log("Entity ID (via SuperTestPlayer):", this.my_derived_player.entity_id);
        console.log("Player Health (via SuperTestPlayer):", this.my_derived_player.health);

        // Access to direct fields of derived struct
        console.log("SuperTestPlayer ability:", this.my_derived_player.special_ability);
        console.log("SuperTestPlayer energy_core:", this.my_derived_player.energy_core);

        // Modification of inherited fields
        this.my_derived_player.health = this.my_derived_player.health - 10.0;
        this.my_derived_player.pos.x = this.my_derived_player.pos.x + 1.0;
        this.my_derived_player.entity_type = "ElitePlayer";
        console.log("Modified SuperTestPlayer health:", this.my_derived_player.health);
        console.log("Modified SuperTestPlayer pos.x:", this.my_derived_player.pos.x);
        console.log("Modified SuperTestPlayer entity_type:", this.my_derived_player.entity_type);

        // Casting: Derived as Base (Upcasting - safe)
        let player_as_entity: GameEntity = this.my_player as GameEntity;
        console.log("TestPlayer as GameEntity name:", player_as_entity.entity_name);

        let super_player_as_player: TestPlayer = this.my_derived_player as TestPlayer;
        console.log("SuperTestPlayer as TestPlayer health:", super_player_as_player.health);

        // Casting: Base as Derived (Downcasting - relies on type assertion)
        let player_to_super_player: SuperTestPlayer = this.my_player as SuperTestPlayer;
        console.log("TestPlayer as SuperTestPlayer (entity_name should be Hero OR default):", player_to_super_player.entity_name);
        console.log("TestPlayer as SuperTestPlayer (ability should be default/empty):", player_to_super_player.special_ability);

        // Here we cast an actual SuperTestPlayer to SuperTestPlayer, which should succeed
        let super_player_roundtrip: SuperTestPlayer = this.my_derived_player as SuperTestPlayer;
        console.log("SuperTestPlayer roundtrip ability (expect Flight):", super_player_roundtrip.special_ability);

        // Member access on a casted variable
        let entity_from_derived: GameEntity = this.my_derived_player as GameEntity;
        entity_from_derived.entity_name = "DerivedEntity";
        console.log("Entity from Derived, name changed (expect DerivedEntity):", entity_from_derived.entity_name);
    }

    // --- Test 5: Dynamic Container Access & Manipulation ---
    public test_dynamic_containers_ops(): void {
        console.log("--- Test Dynamic Containers Ops ---");

        // Array (any[]) - Retrieval & Casting for BigInt/Decimal
        let arr_dyn_val_big: bigint = this.dynamic_array_inferred[4] as bigint;
        arr_dyn_val_big = arr_dyn_val_big * BigInt(2);
        console.log("Dyn Array Elem Op (big):", arr_dyn_val_big);

        let arr_dyn_val_decimal: number = this.dynamic_array_inferred[5] as number;
        arr_dyn_val_decimal = arr_dyn_val_decimal + 0.05;
        console.log("Dyn Array Elem Op (decimal):", arr_dyn_val_decimal);

        // Array (any[]) - Set & Push BigInt/Decimal
        this.dynamic_array_inferred[0] = this.typed_big_int;
        this.dynamic_array_inferred.push(this.local_decimal);
        console.log("Dyn Array Set (big):", this.dynamic_array_inferred[0] as bigint);
        console.log("Dyn Array Push (decimal):", this.dynamic_array_inferred[this.dynamic_array_inferred.length - 1] as number);

        // Map (Map<string, any>) - Retrieval & Casting for BigInt/Decimal
        let map_dyn_val_big: bigint = this.dynamic_map_inferred.get("gamma") as bigint;
        map_dyn_val_big = map_dyn_val_big - BigInt("12345678901234567800");
        console.log("Dyn Map Elem Op (big):", map_dyn_val_big);

        let map_dyn_val_decimal: number = this.dynamic_map_inferred.get("delta") as number;
        map_dyn_val_decimal = map_dyn_val_decimal * 2;
        console.log("Dyn Map Elem Op (decimal):", map_dyn_val_decimal);

        // Map (Map<string, any>) - Set & Insert BigInt/Decimal
        this.dynamic_map_inferred.set("new_big", this.local_big_int);
        this.dynamic_map_inferred.set("new_decimal", this.typed_decimal);
        console.log("Dyn Map Set (new_big):", this.dynamic_map_inferred.get("new_big") as bigint);
        console.log("Dyn Map Set (new_decimal):", this.dynamic_map_inferred.get("new_decimal") as number);

        // Map (Map<string, any>) - Numeric Literal Keys (implicitly String)
        let dyn_map_numeric_key_big: Map<string, any> = new Map([[this.typed_int_default.toString(), this.typed_big_int]]);
        console.log("Dyn Map Num Key Big (20):", dyn_map_numeric_key_big.get("20") as bigint);

        let dyn_map_numeric_key_decimal: Map<string, any> = new Map([[this.typed_float_default.toString(), this.typed_decimal]]);
        console.log("Dyn Map Num Key Dec (30.5):", dyn_map_numeric_key_decimal.get("30.5") as number);
    }

    // --- Test 6: Static Container Access & Manipulation ---
    public test_static_containers_ops(): void {
        console.log("--- Test Static Containers Ops ---");

        // Array[bigint]
        let arr_static_big_elem: bigint = this.static_array_big_int[0];
        arr_static_big_elem = arr_static_big_elem + BigInt(50);
        console.log("Static Array[big] Elem Op:", arr_static_big_elem);

        // Array[number] (decimal)
        let arr_static_decimal_elem: number = this.static_array_decimal[0];
        arr_static_decimal_elem = arr_static_decimal_elem - 0.05;
        console.log("Static Array[decimal] Elem Op:", arr_static_decimal_elem);

        // Map<string, bigint>
        let map_static_big_val: bigint = this.static_map_string_big.get("large_num")!;
        map_static_big_val = map_static_big_val * BigInt(2);
        console.log("Static Map<string:big> Elem Op:", map_static_big_val);

        // Map<string, number> (decimal)
        let map_static_decimal_val: number = this.static_map_string_decimal.get("price")!;
        map_static_decimal_val = map_static_decimal_val + 0.01;
        console.log("Static Map<string:decimal> Elem Op:", map_static_decimal_val);

        // Map<number, number> - Retrieval with bigint key (conversion!)
        let big_to_uint8_key_float_val: number = this.static_map_uint_8_float.get(Number(this.typed_big_int)) || 0.0;
        console.log("Static Map<uint_8:float> Get with big key:", big_to_uint8_key_float_val);

        // Polymorphic access in Static Array[GameEntity]
        let base_entity: GameEntity = this.static_array_entities[0];
        console.log("Static Array[Entity] base_entity name:", base_entity.entity_name);

        let player_as_entity_from_array: GameEntity = this.static_array_entities[1];
        console.log("Static Array[Entity] player_as_entity_from_array name:", player_as_entity_from_array.entity_name);

        let super_player_as_entity_from_array: GameEntity = this.static_array_entities[2];
        console.log("Static Array[Entity] super_player_as_entity_from_array name:", super_player_as_entity_from_array.entity_name);

        // Downcasting from Array[GameEntity] to derived types
        let casted_player: TestPlayer = this.static_array_entities[1] as TestPlayer;
        console.log("Static Array[Entity] Casted Player health:", casted_player.health);

        let casted_super_player: SuperTestPlayer = this.static_array_entities[2] as SuperTestPlayer;
        console.log("Static Array[Entity] Casted SuperPlayer ability:", casted_super_player.special_ability);

        let incompatible_downcast: SuperTestPlayer = this.static_array_entities[0] as SuperTestPlayer;
        console.log("Static Array[Entity] Incompatible Downcast name:", incompatible_downcast.entity_name);
    }

    // =====================================================
    // FUNCTION: init & update
    // Orchestrates tests
    // =====================================================
    public init(): void {
        console.log("--- START TS MEGA TEST SUITE ---");
        this.test_primitive_operations();
        this.test_explicit_casting();
        this.test_assignments();
        this.test_struct_inheritance_and_casting();
        this.test_static_containers_ops();
        this.test_dynamic_containers_ops();
        console.log("--- ALL TS TESTS COMPLETE ---");

   

    }

    public update(): void {
        this.test_primitive_operations();
        this.test_explicit_casting();
        this.test_assignments();
        this.test_struct_inheritance_and_casting();
        this.test_dynamic_containers_ops();
        this.test_static_containers_ops();
    }
}

