using Perro;

public class Player : Node2D
{
    [ThisIsAnAttribute]
    public float speed = 200.0;
    public int health = 1;

    public void Init()
    {
        speed = 10.0;
        Console.WriteLine("Player initialized!");
    }

    public void Update()
    {
        TakeDamage(24);
    }
    
    [DamageFunc]
    public void TakeDamage(int amount)
    {
        health -= amount;
        Console.WriteLine("Took damage!");
    }
}

