void generics() {
    var map = new Gee.HashMap<string, Gee.ArrayList<int>>();
    var list = new Gee.ArrayList<Widget?>();
    Gee.List<Gee.List<int>> nested = null;
    var x = compare<int>(a, b);
    var lt = a < b;
    var cmp = a < b && c > d;
}

public class Box<T, U> : Object {
    public T first;
    public U second;

    public V transform<V>(T input) {
        return (V) input;
    }
}
