[CCode (cname = "external_name", has_target = false)]
public delegate int NativeFunc(int x);

[GtkTemplate (ui = "/org/example/window.ui")]
public class TWindow : Gtk.ApplicationWindow {
    [GtkChild]
    private unowned Gtk.Button ok_button;

    [GtkCallback]
    private void on_clicked() {
    }

    [CCode (instance_pos = -1)]
    public void handler([CCode (type = "gpointer")] Object source) {
    }
}

[DBus (name = "org.example.Iface")]
public interface Iface : Object {
    public abstract async void go() throws DBusError, IOError;
}
