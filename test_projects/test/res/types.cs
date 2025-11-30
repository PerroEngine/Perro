using Perro;

// =====================================================
// TOP-LEVEL EXPOSED & INTERNAL VARIABLES (DEPENDENCY ORDERED)
// =====================================================

public class Types : Node2D
{
    // --- Primitives: Default, Specific Widths, BigInt, Decimal ---
    public int untyped_num_default = 10;
    public int typed_int_default = 20;
    public sbyte typed_int_8 = -120;
    public long typed_int_64 = 1_000_000_000_000;
    public ushort typed_uint_16 = 60000;
    public float typed_float_default = 30.5f;
    public double typed_float_64 = 123.456_789;
    public BigInteger typed_big_int = BigInteger.Parse("12345678901234567890");
    public decimal typed_decimal = 987.6543210987654321m;
    public string typed_string = "HelloPerro";

    // Local Primitives (defined before custom structs that might use them)
    private int local_int = 5;
    private byte local_uint_8 = 250;
    private float local_float = 2.5f;
    private string local_string = "LocalString";
    private BigInteger local_big_int = BigInteger.Parse("1000000000000000000");
    private decimal local_decimal = 12.34567890123456789m;

    // --- Custom Struct Definitions (must precede their usage) ---
    public class TestVector
    {
        public float x;
        public float y;
    }

    public class GameEntity
    {
        public int entity_id;
        public string entity_name;
        public string entity_type;
    }

    public class TestPlayer : GameEntity // TestPlayer inherits GameEntity fields directly
    {
        public TestVector pos;
        public float health;
        public sbyte mana;
    }

    public class SuperTestPlayer : TestPlayer // SuperTestPlayer inherits TestPlayer (and GameEntity) fields directly
    {
        public string special_ability;
        public decimal energy_core;
    }

    // --- Custom Struct Instances (DEFINED IN ORDER OF DEPENDENCY) ---
    public GameEntity my_base_entity = new GameEntity { entity_id = 100, entity_name = "NPC_Guard", entity_type = "NPC" };

    // CORRECTED: TestPlayer initializer is flat, setting all direct & inherited fields
    public TestPlayer my_player = new TestPlayer
    {
        entity_id = 1,      // Inherited from GameEntity
        entity_name = "Hero", // Inherited from GameEntity
        entity_type = "Player", // Inherited from GameEntity
        pos = new TestVector { x = 0.0f, y = 0.0f }, // Direct field
        health = 100.0f,     // Direct field
        mana = 50           // Direct field
    };

    // Local Player Instance (defined before containers that use it)
    private TestPlayer other_player = new TestPlayer
    {
        entity_id = 2,
        entity_name = "Sidekick",
        entity_type = "Player",
        pos = new TestVector { x = 5.0f, y = 10.0f },
        health = 80.0f,
        mana = 30
    };

    // CORRECTED: SuperTestPlayer initializer is flat, setting all direct & inherited fields
    public SuperTestPlayer my_derived_player = new SuperTestPlayer
    {
        entity_id = 3,      // Inherited from GameEntity
        entity_name = "SuperHero", // Inherited from GameEntity
        entity_type = "SuperPlayer", // Inherited from GameEntity
        pos = new TestVector { x = 10.0f, y = 10.0f }, // Inherited from TestPlayer
        health = 200.0f,     // Inherited from TestPlayer
        mana = 100,         // Inherited from TestPlayer
        special_ability = "Flight", // Direct field
        energy_core = 99.9m  // Direct field
    };

    // Local SuperTestPlayer Instance (defined before containers that use it)
    private SuperTestPlayer my_derived_player_var = new SuperTestPlayer
    {
        entity_id = 4,
        entity_name = "LocalHero",
        entity_type = "LocalPlayer",
        pos = new TestVector { x = 20.0f, y = 20.0f },
        health = 180.0f,
        mana = 80,
        special_ability = "Speed",
        energy_core = 80.0m
    };

    // --- Dynamic Containers (always object[] / Dictionary<string, object> in C#) ---
    public object[] dynamic_array_inferred = new object[] { 1, "two", 3.0, null, null, null }; // Mix of literals & var (will be initialized in constructor)
    public Dictionary<string, object> dynamic_map_inferred = new Dictionary<string, object>(); // Mix of literals & var (will be initialized in constructor)

    // Annotated containers now correctly reference defined variables
    public object[] dynamic_array_annotated; // Will be initialized in constructor
    public Dictionary<string, object> dynamic_map_annotated; // Will be initialized in constructor

    // --- Statically Typed Arrays ---
    public int[] static_array_int = new int[] { 10, 20, 30 };
    public ushort[] static_array_uint_16 = new ushort[] { 1000, 2000 };
    public string[] static_array_string = new string[] { "one", "two", "three" };
    public double[] static_array_float_64 = new double[] { 1.11, 2.22, 3.33 };
    public BigInteger[] static_array_big_int; // Will be initialized in constructor
    public decimal[] static_array_decimal; // Will be initialized in constructor

    // --- Statically Typed Maps ---
    public Dictionary<string, long> static_map_string_int_64 = new Dictionary<string, long> { { "level", 10_000_000_000 }, { "score", 1_000_000_000 } };
    public Dictionary<int, string> static_map_int_string = new Dictionary<int, string> { { 1, "gold" }, { 2, "silver" } };
    public Dictionary<byte, float> static_map_uint_8_float = new Dictionary<byte, float> { { 50, 0.5f }, { 100, 1.5f } };
    public Dictionary<string, BigInteger> static_map_string_big; // Will be initialized in constructor
    public Dictionary<string, decimal> static_map_string_decimal = new Dictionary<string, decimal> { { "price", 19.99m }, { "tax", 1.50m } };

    // --- Statically Typed Containers of Custom Structs (using correctly initialized instances) ---
    public GameEntity[] static_array_entities; // Will be initialized in constructor
    public TestPlayer[] static_array_players; // Will be initialized in constructor
    public Dictionary<string, TestPlayer> static_map_players; // Will be initialized in constructor
    public Dictionary<string, SuperTestPlayer> static_map_super_players; // Will be initialized in constructor

    // Constructor to initialize fields that depend on other fields
    public Types()
    {
        // Initialize dynamic_array_inferred with variable references
        dynamic_array_inferred = new object[] { 1, "two", 3.0, untyped_num_default, typed_big_int, typed_decimal };

        // Initialize dynamic_map_inferred
        dynamic_map_inferred = new Dictionary<string, object>
        {
            { "alpha", 1 },
            { "beta", typed_string },
            { "gamma", typed_big_int },
            { "delta", typed_decimal }
        };

        // Initialize dynamic_array_annotated
        dynamic_array_annotated = new object[] { typed_int_default, typed_float_64, my_derived_player_var, typed_big_int };

        // Initialize dynamic_map_annotated
        dynamic_map_annotated = new Dictionary<string, object>
        {
            { "char", "A" },
            { "num", typed_int_8 },
            { "big_val", local_big_int }
        };

        // Initialize static_array_big_int
        static_array_big_int = new BigInteger[] { 100, 200, typed_big_int };

        // Initialize static_array_decimal
        static_array_decimal = new decimal[] { 5.5m, 6.6m, typed_decimal };

        // Initialize static_map_string_big
        static_map_string_big = new Dictionary<string, BigInteger>
        {
            { "large_num", BigInteger.Parse("9999999999999999999") },
            { "another_large", new BigInteger(1000) }
        };

        // Initialize static_array_entities
        static_array_entities = new GameEntity[] { my_base_entity, my_player, my_derived_player };

        // Initialize static_array_players
        static_array_players = new TestPlayer[] { my_player, other_player };

        // Initialize static_map_players
        static_map_players = new Dictionary<string, TestPlayer>
        {
            { "main", my_player },
            { "other", other_player }
        };

        // Initialize static_map_super_players
        static_map_super_players = new Dictionary<string, SuperTestPlayer>
        {
            { "super", my_derived_player },
            { "local_super", my_derived_player_var }
        };
    }

    // =====================================================
    // TEST SUITE FUNCTIONS
    // =====================================================

    // --- Test 1: Primitive Operations (literals, variables, promotion) ---
    public void TestPrimitiveOperations()
    {
        Console.WriteLine("--- Test Primitive Operations ---");

        // Variable + Literal
        int res_int = typed_int_default + 10;
        BigInteger res_big = typed_big_int + 1000;
        decimal res_decimal = typed_decimal + 1.000001m;
        Console.WriteLine($"Var + Lit: {res_int}, {res_big}, {res_decimal}");

        // Variable + Variable
        BigInteger res_big_var = typed_big_int + local_big_int;
        decimal res_decimal_var = typed_decimal + local_decimal;
        Console.WriteLine($"Var + Var (Big/Dec): {res_big_var}, {res_decimal_var}");

        // Type Promotion with BigInt/Decimal
        double prom_float_big = (double)typed_int_64 + (double)typed_big_int;
        double prom_float_decimal = typed_float_64 + (double)typed_decimal;
        BigInteger prom_big_int = new BigInteger(typed_int_64) + typed_big_int;
        Console.WriteLine($"Promotion (Big/Dec): {prom_float_big}, {prom_float_decimal}, {prom_big_int}");
    }

    // --- Test 2: Explicit Type Casting ---
    public void TestExplicitCasting()
    {
        Console.WriteLine("--- Test Explicit Casting ---");

        // Primitive Variable to Primitive Variable (various widths, Big/Decimal)
        BigInteger int64_to_big = new BigInteger(typed_int_64);
        int big_to_int = (int)typed_big_int;
        decimal float_to_decimal = (decimal)typed_float_default;
        double decimal_to_float_64 = (double)typed_decimal;
        ushort string_to_uint16 = ushort.Parse("65530");
        string big_to_string = typed_big_int.ToString();
        Console.WriteLine($"Numeric Casts: {int64_to_big}, {big_to_int}, {float_to_decimal}, {decimal_to_float_64}, {string_to_uint16}, {big_to_string}");

        // Dynamic Value to Primitive (various widths, Big/Decimal)
        BigInteger dyn_val_big = (BigInteger)dynamic_array_inferred[4];
        decimal dyn_val_decimal = (decimal)dynamic_array_inferred[5];
        Console.WriteLine($"Dyn->Big/Dec Casts: {dyn_val_big}, {dyn_val_decimal}");

        // Cast chain and operations
        BigInteger casted_and_op_big = (BigInteger)dynamic_array_inferred[0] + typed_big_int;
        Console.WriteLine($"Casted & Op (Big): {casted_and_op_big}");
    }

    // --- Test 3: Assignments (Simple & Compound) ---
    public void TestAssignments()
    {
        Console.WriteLine("--- Test Assignments (Simple & Compound) ---");

        // Simple Assignment (=)
        BigInteger assign_big_lit = 999;
        decimal assign_decimal_var = typed_decimal;
        Console.WriteLine($"Simple Assign (Big/Dec): {assign_big_lit}, {assign_decimal_var}");

        // Compound Assignment (+=, -=) with BigInt/Decimal
        BigInteger comp_big = 100;
        comp_big += 50;
        Console.WriteLine($"Comp Assign big += {comp_big}");

        decimal comp_decimal = 20.0m;
        comp_decimal -= 5.5m;
        Console.WriteLine($"Comp Assign decimal -= {comp_decimal}");

        // Assignments with Type Promotion/Casting
        decimal assign_prom_decimal = (decimal)typed_int_default;
        assign_prom_decimal += (decimal)typed_float_default;
        Console.WriteLine($"Assign Promo decimal: {assign_prom_decimal}");

        // Member Assignment with BigInt/Decimal (for existing float fields with casts)
        my_player.pos.x = (float)typed_big_int;
        my_player.health = (float)typed_decimal;
        Console.WriteLine($"Member Assign (Big/Dec to float): {my_player.pos.x}, {my_player.health}");
    }

    // --- Test 4: Struct Inheritance, Member Access, Casting between Base/Derived ---
    public void TestStructInheritanceAndCasting()
    {
        Console.WriteLine("--- Test Struct Inheritance & Casting ---");

        // Direct Access to inherited fields (as Pup implies)
        Console.WriteLine($"Player name (via SuperTestPlayer): {my_derived_player.entity_name}"); // Correct, it's entity_name
        Console.WriteLine($"Entity ID (via SuperTestPlayer): {my_derived_player.entity_id}");     // Correct, it's entity_id
        Console.WriteLine($"Player Health (via SuperTestPlayer): {my_derived_player.health}");     // Correct, it's health

        // Access to direct fields of derived struct
        Console.WriteLine($"SuperTestPlayer ability: {my_derived_player.special_ability}");
        Console.WriteLine($"SuperTestPlayer energy_core: {my_derived_player.energy_core}");

        // Modification of inherited fields
        my_derived_player.health = my_derived_player.health - 10.0f; // health is a direct field for Pup
        my_derived_player.pos.x = my_derived_player.pos.x + 1.0f;    // pos.x is direct
        my_derived_player.entity_type = "ElitePlayer"; // entity_type is direct
        Console.WriteLine($"Modified SuperTestPlayer health: {my_derived_player.health}");
        Console.WriteLine($"Modified SuperTestPlayer pos.x: {my_derived_player.pos.x}");
        Console.WriteLine($"Modified SuperTestPlayer entity_type: {my_derived_player.entity_type}");

        // Casting: Derived as Base (Upcasting - safe)
        GameEntity player_as_entity = my_player; // Implicit upcast in C#
        Console.WriteLine($"TestPlayer as GameEntity name: {player_as_entity.entity_name}");

        TestPlayer super_player_as_player = my_derived_player; // Implicit upcast in C#
        Console.WriteLine($"SuperTestPlayer as TestPlayer health: {super_player_as_player.health}");

        // Casting: Base as Derived (Downcasting - requires explicit cast)
        // Pup `my_player` is `TestPlayer`. Casting to `SuperTestPlayer` (its derived class) should result in default if type mismatch.
        SuperTestPlayer player_to_super_player = my_player as SuperTestPlayer; // Will be null if incompatible
        if (player_to_super_player != null)
        {
            Console.WriteLine($"TestPlayer as SuperTestPlayer (entity_name should be Hero OR default): {player_to_super_player.entity_name}");
            Console.WriteLine($"TestPlayer as SuperTestPlayer (ability should be default/empty): {player_to_super_player.special_ability}");
        }
        else
        {
            Console.WriteLine("TestPlayer as SuperTestPlayer: null (incompatible cast)");
        }

        // Here we cast an actual SuperTestPlayer to SuperTestPlayer, which should succeed
        SuperTestPlayer super_player_roundtrip = my_derived_player as SuperTestPlayer;
        Console.WriteLine($"SuperTestPlayer roundtrip ability (expect Flight): {super_player_roundtrip.special_ability}");

        // Member access on a casted variable
        GameEntity entity_from_derived = my_derived_player; // Implicit upcast
        entity_from_derived.entity_name = "DerivedEntity";
        Console.WriteLine($"Entity from Derived, name changed (expect DerivedEntity): {entity_from_derived.entity_name}");
    }

    // --- Test 5: Dynamic Container Access & Manipulation ---
    public void TestDynamicContainersOps()
    {
        Console.WriteLine("--- Test Dynamic Containers Ops ---");

        // Array (object[]) - Retrieval & Casting for BigInt/Decimal
        BigInteger arr_dyn_val_big = (BigInteger)dynamic_array_inferred[4];
        arr_dyn_val_big *= 2;
        Console.WriteLine($"Dyn Array Elem Op (big): {arr_dyn_val_big}");

        decimal arr_dyn_val_decimal = (decimal)dynamic_array_inferred[5];
        arr_dyn_val_decimal += 0.05m;
        Console.WriteLine($"Dyn Array Elem Op (decimal): {arr_dyn_val_decimal}");

        // Array (object[]) - Set & Push BigInt/Decimal
        dynamic_array_inferred[0] = typed_big_int;
        // For arrays, we need to resize or use List
        Array.Resize(ref dynamic_array_inferred, dynamic_array_inferred.Length + 1);
        dynamic_array_inferred[dynamic_array_inferred.Length - 1] = local_decimal;
        Console.WriteLine($"Dyn Array Set (big): {(BigInteger)dynamic_array_inferred[0]}");
        Console.WriteLine($"Dyn Array Push (decimal): {(decimal)dynamic_array_inferred[dynamic_array_inferred.Length - 1]}");

        // Map (Dictionary<string, object>) - Retrieval & Casting for BigInt/Decimal
        BigInteger map_dyn_val_big = (BigInteger)dynamic_map_inferred["gamma"];
        map_dyn_val_big -= BigInteger.Parse("12345678901234567800");
        Console.WriteLine($"Dyn Map Elem Op (big): {map_dyn_val_big}");

        decimal map_dyn_val_decimal = (decimal)dynamic_map_inferred["delta"];
        map_dyn_val_decimal *= 2;
        Console.WriteLine($"Dyn Map Elem Op (decimal): {map_dyn_val_decimal}");

        // Map (Dictionary<string, object>) - Set & Insert BigInt/Decimal
        dynamic_map_inferred["new_big"] = local_big_int;
        dynamic_map_inferred["new_decimal"] = typed_decimal;
        Console.WriteLine($"Dyn Map Set (new_big): {(BigInteger)dynamic_map_inferred["new_big"]}");
        Console.WriteLine($"Dyn Map Set (new_decimal): {(decimal)dynamic_map_inferred["new_decimal"]}");

        // Map (Dictionary<string, object>) - Numeric Literal Keys (implicitly String)
        Dictionary<string, object> dyn_map_numeric_key_big = new Dictionary<string, object> { { typed_int_default.ToString(), typed_big_int } }; // Key "20"
        Console.WriteLine($"Dyn Map Num Key Big (20): {(BigInteger)dyn_map_numeric_key_big["20"]}");

        Dictionary<string, object> dyn_map_numeric_key_decimal = new Dictionary<string, object> { { typed_float_default.ToString(), typed_decimal } }; // Key "30.5"
        Console.WriteLine($"Dyn Map Num Key Dec (30.5): {(decimal)dyn_map_numeric_key_decimal["30.5"]}");
    }

    // --- Test 6: Static Container Access & Manipulation ---
    public void TestStaticContainersOps()
    {
        Console.WriteLine("--- Test Static Containers Ops ---");

        // Array[big]
        BigInteger arr_static_big_elem = static_array_big_int[0];
        arr_static_big_elem += 50;
        Console.WriteLine($"Static Array[big] Elem Op: {arr_static_big_elem}");

        // Array[decimal]
        decimal arr_static_decimal_elem = static_array_decimal[0];
        arr_static_decimal_elem -= 0.05m;
        Console.WriteLine($"Static Array[decimal] Elem Op: {arr_static_decimal_elem}");

        // Map<[string: big]>
        BigInteger map_static_big_val = static_map_string_big["large_num"];
        map_static_big_val *= 2;
        Console.WriteLine($"Static Map<string:big> Elem Op: {map_static_big_val}");

        // Map<[string: decimal]>
        decimal map_static_decimal_val = static_map_string_decimal["price"];
        map_static_decimal_val += 0.01m;
        Console.WriteLine($"Static Map<string:decimal> Elem Op: {map_static_decimal_val}");

        // Map<[uint_8: float]> - Retrieval with `big` key (conversion!)
        byte big_to_uint8_key = (byte)typed_big_int; // big -> uint_8 key
        float big_to_uint8_key_float_val = static_map_uint_8_float.ContainsKey(big_to_uint8_key) 
            ? static_map_uint_8_float[big_to_uint8_key] 
            : 0.0f; // Expect 0.0 (if typed_big_int as u8 wraps/saturates to a key not present)
        Console.WriteLine($"Static Map<uint_8:float> Get with big key: {big_to_uint8_key_float_val}");

        // Polymorphic access in Static Array[GameEntity]
        GameEntity base_entity = static_array_entities[0]; // GameEntity
        Console.WriteLine($"Static Array[Entity] base_entity name: {base_entity.entity_name}");

        GameEntity player_as_entity_from_array = static_array_entities[1]; // TestPlayer as GameEntity
        Console.WriteLine($"Static Array[Entity] player_as_entity_from_array name: {player_as_entity_from_array.entity_name}");

        GameEntity super_player_as_entity_from_array = static_array_entities[2]; // SuperTestPlayer as GameEntity
        Console.WriteLine($"Static Array[Entity] super_player_as_entity_from_array name: {super_player_as_entity_from_array.entity_name}");

        // Downcasting from Array[GameEntity] to derived types
        TestPlayer casted_player = static_array_entities[1] as TestPlayer; // Should succeed
        Console.WriteLine($"Static Array[Entity] Casted Player health: {casted_player.health}");

        SuperTestPlayer casted_super_player = static_array_entities[2] as SuperTestPlayer; // Should succeed
        Console.WriteLine($"Static Array[Entity] Casted SuperPlayer ability: {casted_super_player.special_ability}");

        SuperTestPlayer incompatible_downcast = static_array_entities[0] as SuperTestPlayer; // GameEntity as SuperTestPlayer (fail)
        if (incompatible_downcast != null)
        {
            Console.WriteLine($"Static Array[Entity] Incompatible Downcast name: {incompatible_downcast.entity_name}"); // entity_name is a field on SuperTestPlayer
        }
        else
        {
            Console.WriteLine("Static Array[Entity] Incompatible Downcast: null (incompatible cast)");
        }
    }

    // =====================================================
    // FUNCTION: Init & Update
    // Orchestrates tests
    // =====================================================
    public void Init()
    {
        Console.WriteLine("--- START CSharp MEGA TEST SUITE ---");
        TestPrimitiveOperations();
        TestExplicitCasting();
        TestAssignments();
        TestStructInheritanceAndCasting();
        TestDynamicContainersOps();
        TestStaticContainersOps();
        Console.WriteLine("--- ALL CSharp TESTS COMPLETE ---");
    }

    public void Update()
    {
        TestPrimitiveOperations();
        TestExplicitCasting();
        TestAssignments();
        TestStructInheritanceAndCasting();
        TestDynamicContainersOps();
        TestStaticContainersOps();
    }
}

