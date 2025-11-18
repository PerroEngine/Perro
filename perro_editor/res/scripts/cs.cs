using Perro;

public class Player : Node2D
{
    [Expose]
    public float speed = 200.0;
    
    public int health = 100;
    
    private string playerName = "Hero";

    long james = 1000000000000;

    public class TestClass {
        int b = 100;
        float f = 100.0;

    }

    
    public void Init()
    {
        health = 100;
        speed = 200.0;
        speed += 50.0;
        Console.WriteLine("Player initialized!");
    }

    public void Update()
    {
        TakeDamage(2);
    }
    
    public void TakeDamage(int amount)
    {
        health -= amount;
        Console.WriteLine("Took damage!");
    }
}