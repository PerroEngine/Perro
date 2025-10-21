using Perro;

public class CSharp : Node {
    
    private class Player {
        public int hp;
        public string name;
    }

        private class Player2 : Player {
        public int hp1;
        public string name1;
    }


    private class Player3: Player2 {
        public int hp2;
        public string name;
    }


    void Init() {
        Console.WriteLine("Hello World I am csharp.cs");
    }

    void Update(float delta) {
    }

    void Fart() {
        Console.WriteLine("Fart");
    }
}