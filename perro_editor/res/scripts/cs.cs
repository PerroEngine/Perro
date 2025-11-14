using Perro;

public class Player : Node3D
{
    [Expose]
    public float speed = 200.0f;
    
    public int health = 100;
    
    private string playerName = "Hero";

    
    public void Init()
    {
        health = 100;
        speed = 200.0f;
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