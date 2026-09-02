public delegate bool Filter(int item);
public delegate void Callback() throws Error;

public class Emitter : Object {
    public signal void changed(int new_value);
    public virtual signal void done() {
        cleanup();
    }

    public void fire() {
        changed(42);
    }
}
